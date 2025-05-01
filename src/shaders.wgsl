const FLOAT_MAX: f32 = 3.40282346638528859812e+38;
const EPSILON: f32 = 0.0005;
const PI: f32 = 3.14159265358;

fn equal_zero(a: f32) -> bool {
    return abs(a) < EPSILON;
}

// a slightly modified version of the "One-at-a-Time Hash" function by Bob Jenkins
// see https://www.burtleburtle.net/bob/hash/doobs.html
fn jenkins_hash(i: u32) -> u32 {
    var x = i;
    x += x << 10u;
    x ^= x >> 6u;
    x += x << 3u;
    x ^= x >> 11u;
    x += x << 15u;
    return x;
}

struct RNG {
    state: u32,
    cached_normal_sample: f32,
    has_cached: bool,
};
var<private> rng: RNG;

// the 32-bit "xor" function from Marsaglia G., "Xorshift RNGs", Section 3
fn xorshift32() -> u32 {
    var x = rng.state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    rng.state = x;
    return x;
}

fn init_rng(pixel: vec2u) {
    let time_seed = u32(uniforms.elapsed_seconds * 1000.0);
    let spatial_seed = pixel.x + pixel.y * uniforms.width;
    let seed = spatial_seed ^ jenkins_hash(uniforms.frame_count) ^ jenkins_hash(time_seed);
    rng.state = jenkins_hash(seed);
    rng.cached_normal_sample = 0.0;
    rng.has_cached = false;
}

fn rand() -> f32 {
    return bitcast<f32>(0x3f800000u | (xorshift32() >> 9u)) - 1.0;
}

fn rand_normal() -> f32 {
    if rng.has_cached {
        rng.has_cached = false;
        return rng.cached_normal_sample;
    }

    let u1 = max(rand(), 1e-6); // avoid log(0)
    let u2 = rand();

    // Box-Muller transform
    // mean = 0, std dev = 1
    let mag = sqrt(-2.0 * log(u1));
    let theta = 2.0 * PI * u2;
    let z0 =  mag * cos(theta);

    rng.cached_normal_sample = mag * sin(theta);
    rng.has_cached = true;
    return z0;
}

fn rand_sphere() -> vec3f {
    return normalize(vec3f(
        rand_normal(),
        rand_normal(),
        rand_normal()
    ));
}

fn rand_circle() -> vec2f {
    let angle = rand() * 2.0 * PI;
    let point_on_circle = vec2f(cos(angle), sin(angle));
    return point_on_circle * sqrt(rand());
}

fn rand_polygon(n: i32) -> vec2f {
    let angle = 2.0 * PI / f32(n);
    let t = rand() * 2.0 * PI;
    let r = sqrt(rand()); // radial falloff
    let a0 = floor(t / angle) * angle;
    let a1 = a0 + angle;
    let f = (t - a0) / angle;

    let v0 = vec2f(cos(a0), sin(a0));
    let v1 = vec2f(cos(a1), sin(a1));
    return mix(v0, v1, f) * r;
}

struct Camera {
    width: f32,
    fov: f32,
    focus_distance: f32,
    apeture: f32,
    diverge_strength: f32,
    max_ray_bounces: u32,
    // ^ size 32, align 4
    position: vec3f,
    direction: vec3f,
    // ^ size 32, align 16
}


struct Uniforms {
    camera: Camera,
    // ^ size 64, align 16
    width: u32,
    height: u32,
    elapsed_seconds: f32,
    frame_count: u32,
    gamma_correction: f32,
    // ^ size 32, align 4
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var radiance_samples_old: texture_2d<f32>;
@group(0) @binding(2) var radiance_samples_new: texture_storage_2d<rgba32float, write>;

struct Ray {
    origin: vec3f,
    direction: vec3f,
}

struct HitInfo {
    did_hit: bool,
    point: vec3f,
    normal: vec3f,
    distance: f32,
    inside_object: bool,
    material_id: u32,
}

struct Material {
    color: vec3f,
    roughness_or_ior: f32,
    emission_strength: f32,
    volume_density: f32,
}

struct Sphere {
    center: vec3f,
    radius: f32,
    material_id: u32,
}

var<private> materials: array<Material, 32>;
var<private> spheres: array<Sphere, 64>;
var<private> sphere_count: u32 = 6;

fn sky_color(ray: Ray) -> vec3f {
    let t = 0.5 * (normalize(ray.direction).y + 1.0);
    return (1.0 - t) * vec3(1.0) + t * vec3(0.3, 0.5, 1.0);
    // return vec3f(0.0);
}

fn new_ray(pos: vec4f) -> Ray {
    let aspect_ratio = f32(uniforms.width) / f32(uniforms.height);

    let camera_right_direction = -cross(uniforms.camera.direction, vec3f(0.0, 1.0, 0.0));
    let camera_up_direction = cross(uniforms.camera.direction, camera_right_direction);

    // offset ray origin for defocusing effect
    // note that the jitter is in a circle pattern
    // that also means the created boked shape is circle
    // to create other bokeh shapes, just change the rand_circle() into
    // rand_hexagon() or something similar
    let defocus_jitter = vec3f(rand_circle() * uniforms.camera.apeture * 0.5, 0.0);
    let origin_offset = camera_up_direction * defocus_jitter.y + camera_right_direction * defocus_jitter.x;
    let ray_origin = uniforms.camera.position + origin_offset;

    // random jitter for anti-aliasing
    let jitter = rand_circle() * uniforms.camera.diverge_strength;
    var uv = pos.xyz / vec3f(f32(uniforms.width - 1), f32(uniforms.height - 1), 0.0);
    uv = (2.0 * uv - vec3f(1.0)) * vec3f(aspect_ratio, -1.0, 0.0);
    uv = camera_up_direction * (uv.y + jitter.y) + camera_right_direction * (uv.x + jitter.x);
    
    let focal_length = uniforms.camera.width * 0.5 / tan(uniforms.camera.fov * 0.5);
    let focus_direction = normalize(uv + uniforms.camera.direction * focal_length);

    return Ray (
        ray_origin,
        normalize(focus_direction * uniforms.camera.focus_distance - origin_offset),
    );
}

fn intersect_sphere(ray: Ray, sphere: Sphere) -> HitInfo {
    var hit = HitInfo(
        false,
        vec3f(0.0),
        vec3f(0.0),
        0.0,
        false,
        0,
    );

    let v = ray.origin - sphere.center;
    let a = dot(ray.direction, ray.direction);
    let b = dot(v, ray.direction);
    let c = dot(v, v) - sphere.radius * sphere.radius;

    let dsc = b * b - a * c;

    // if the ray origin is on (or very near) the sphere surface
    // and is going outward then there must be no hit
    if equal_zero(c) && b >= 0 {
        return hit;
    }

    if dsc < 0.0 {
        return hit;
    }

    hit.inside_object = c < EPSILON;

    let sqrt_dsc = sqrt(dsc);
    let recip_a = 1.0 / a;
    let t1 = (-b - sqrt_dsc) * recip_a;
    let t2 = (-b + sqrt_dsc) * recip_a;
    hit.distance = select(t1, t2, t1 <= EPSILON);
    if hit.distance <= EPSILON {
        return hit;
    }

    hit.point = ray.origin + ray.direction * hit.distance;
    hit.normal = (hit.point - sphere.center) / sphere.radius;
    if hit.inside_object {
        hit.normal *= -1;
    }
    hit.material_id = sphere.material_id;

    hit.did_hit = true;
    return hit;
}

fn get_ray_collision(ray: Ray) -> HitInfo {
    var closest_hit: HitInfo;
    closest_hit.distance = FLOAT_MAX;

    for(var i = 0u; i < sphere_count; i += 1u) {
        let hit = intersect_sphere(ray, spheres[i]);
        if !hit.did_hit {
            continue;
        }

        if hit.distance < closest_hit.distance {
            closest_hit= hit;
        }
    }

    return closest_hit;
}

fn path_trace(ray_pos: vec4f) -> vec3f {
    var incomming_light = vec3f(0.0, 0.0, 0.0);
    var ray_color = vec3f(1.0);

    var ray = new_ray(ray_pos);

    var surrounding_volume_density = 0.0;
    var surrounding_volume_radiance = vec3f(0.0);

    // check surrounding
    for(var i = 0u; i < sphere_count; i += 1u) {
        let d = ray.origin - spheres[i].center;
        if dot(d, d) < spheres[i].radius * spheres[i].radius {
            let material = materials[spheres[i].material_id];
            surrounding_volume_density += material.volume_density;
            surrounding_volume_radiance += material.emission_strength * material.color;
        }
    }

    var bounces = 0u;
    while bounces < uniforms.camera.max_ray_bounces {
        let hit = get_ray_collision(ray);

        if !hit.did_hit {
            incomming_light += ray_color * sky_color(ray);
            break;
        }

        let material = materials[hit.material_id];

        if surrounding_volume_density > 0.0 {
            let scattering_distance = -log(rand()) / surrounding_volume_density;

            if scattering_distance < hit.distance {
                // hit the particle
                let transmittance = exp(-surrounding_volume_density * scattering_distance);
                let radiance = surrounding_volume_radiance * (1.0 - transmittance) / surrounding_volume_density;
                incomming_light += ray_color * radiance;
                ray_color *= transmittance;
                ray.origin += ray.direction * scattering_distance;
                ray.direction = rand_sphere();
                bounces += 1;
                continue;
            }
        }

        if material.volume_density < 1.0 {
            if hit.inside_object {
                surrounding_volume_density -= material.volume_density;
                surrounding_volume_radiance -= material.emission_strength * material.color;
            } else {
                surrounding_volume_density += material.volume_density;
                surrounding_volume_radiance += material.emission_strength * material.color;
            }
            ray.origin = hit.point + ray.direction * EPSILON;
            continue;
        }

        // ray_color *= hit.normal * 0.5 + vec3f(0.5);
        ray_color *= material.color;
        incomming_light += ray_color * material.emission_strength;

        if material.roughness_or_ior >= 0.0 {
            // calculate scattering direction
            let diffuse_direction = normalize(hit.normal + (1.0 - EPSILON) * rand_sphere());
            let specular_direction = reflect(ray.direction, hit.normal);
            ray.direction = mix(specular_direction, diffuse_direction, material.roughness_or_ior);
        } else {
            let incident_dot_normal = dot(ray.direction, hit.normal);
            let cos_theta = abs(incident_dot_normal);

            let ior = -select(material.roughness_or_ior, 1.0 / material.roughness_or_ior, !hit.inside_object);
            let cannot_refract = ior * ior * (1.0 - cos_theta * cos_theta) > 1.0;

            if cannot_refract {
                ray.direction = reflect(ray.direction, hit.normal);
            } else {
                ray.direction = refract(ray.direction, hit.normal, ior);
            }
        }
        ray.origin = hit.point + ray.direction * EPSILON;
        bounces += 1;
    }

    return incomming_light;
}

@fragment
fn fs_display(
    @builtin(position) pos: vec4f,
) -> @location(0) vec4f {

    init_rng(vec2u(pos.xy));

    materials[0] = Material(
        vec3f(0.7, 1.0, 0.3),
        1.0,
        0.0,
        1.0,
    );
    materials[1] = Material(
        vec3f(0.49, 0.25, 0.88),
        0.6,
        0.0,
        1.0,
    );
    materials[2] = Material(
        vec3f(1.0),
        -1.77,
        0.0,
        1.0,
    );
    materials[3] = Material(
        vec3f(0.8, 0.7, 0.8),
        0.6,
        1.0,
        1.0,
    );
    materials[4] = Material(
        vec3f(0.9),
        0.6,
        0.3,
        0.3,
    );

    var sim_time = fract(uniforms.elapsed_seconds) * 2.0;

    spheres[0] = Sphere(
        vec3f(0.0, -100.5, -2.0),
        100.0,
        0,
    );
    spheres[1] = Sphere(
        vec3f(0.5, 3.0, -12.0),
        4.5,
        1,
    );
    spheres[2] = Sphere(
        vec3f(0.0, 0.0, -2.0),
        0.5,
        2,
    );
    spheres[3] = Sphere(
        vec3f(-1.0, -0.1, 2.0 * pow(0.5, sim_time * 5.0) - 5.0),
        0.4,
        3,
    );
    spheres[4] = Sphere(
        vec3f(1.0, 1.0, -4.0),
        1.5,
        4,
    );
    spheres[5] = Sphere(
        vec3f(0.0, 0.0, -0.5),
        0.5,
        1,
    );

    // load previous progress
    var color: vec4f;
    if uniforms.frame_count > 1 {
        color = textureLoad(radiance_samples_old, vec2u(pos.xy), 0);
    } else {
        color = vec4f(0.0);
    }

    // save new progress and render
    let path_traced = vec4f(path_trace(pos), 1.0);
    color += path_traced;
    textureStore(radiance_samples_new, vec2u(pos.xy), color);

    return pow(color / f32(uniforms.frame_count), vec4f(1.0 / uniforms.gamma_correction));
    // return pow(path_traced, vec4f(1.0 / uniforms.gamma_correction));
}

var<private> vertices: array<vec2f, 6> = array<vec2f, 6>(
    vec2f(-1.0,  1.0),
    vec2f(-1.0, -1.0),
    vec2f( 1.0,  1.0),
    vec2f( 1.0,  1.0),
    vec2f(-1.0, -1.0),
    vec2f( 1.0, -1.0),
);

@vertex
fn vs_display(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4f {
    return vec4f(vertices[vid], 0.0, 1.0);
}
