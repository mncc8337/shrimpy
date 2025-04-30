const FLOAT_MAX: f32 = 3.40282346638528859812e+38;
const EPSILON: f32 = 0.0001;
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
}

fn rand() -> f32 {
    return bitcast<f32>(0x3f800000u | (xorshift32() >> 9u)) - 1.0;
}

fn rand_normal() -> f32 {
    let u1 = max(rand(), 1e-7); // avoid log(0)
    let u2 = rand();

    // Box-Muller transform
    let z0 = sqrt(-2.0 * log(u1)) * cos(2.0 * PI * u2);
    return z0; // mean = 0, std dev = 1
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
    let r = pow(rand(), 0.5); // radial falloff
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
    material: Material,
}

struct Material {
    color: vec3f,
    roughness: f32,
}

struct Sphere {
    center: vec3f,
    radius: f32,
    material: Material,
}

alias Scene = array<Sphere, 32>;

fn sky_color(ray: Ray) -> vec3f {
    let t = 0.5 * (normalize(ray.direction).y + 1.0);
    return (1.0 - t) * vec3(1.0) + t * vec3(0.3, 0.5, 1.0);
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
        focus_direction * uniforms.camera.focus_distance - origin_offset,
    );
}

fn intersect_sphere(ray: Ray, sphere: Sphere) -> HitInfo {
    var hit = HitInfo(
        false,
        vec3f(0.0),
        vec3f(0.0),
        0.0,
        false,
        Material(
            vec3f(1.0),
            1.0,
        ),
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
    hit.material = sphere.material;

    hit.did_hit = true;
    return hit;
}

fn get_ray_collision(ray: Ray, scene: Scene, obj_count: u32) -> HitInfo {
    var closest_hit: HitInfo;
    closest_hit.distance = FLOAT_MAX;

    for(var i = 0u; i < obj_count; i += 1u) {
        let hit = intersect_sphere(ray, scene[i]);
        if !hit.did_hit {
            continue;
        }

        if hit.distance < closest_hit.distance {
            closest_hit= hit;
        }
    }

    return closest_hit;
}

fn path_trace(ray_pos: vec4f, scene: Scene, obj_count: u32) -> vec3f {
    var incomming_light = vec3f(0.0, 0.0, 0.0);
    var ray_color = vec3f(1.0);

    var ray = new_ray(ray_pos);

    for(var bounce_i = 0u; bounce_i < uniforms.camera.max_ray_bounces; bounce_i += 1u) {
        let hit = get_ray_collision(ray, scene, obj_count);
        if !hit.did_hit {
            incomming_light += ray_color * sky_color(ray);
            break;
        }

        // ray_color *= vec4f(hit.normal * 0.5 + vec3f(0.5), 1.0);
        ray_color *= hit.material.color;

        // if emits light
        // incomming_light += ray_color * color * h.material.emission_strength;

        // calculate scattering direction
        let diffuse_direction = normalize(hit.normal + rand_sphere());
        let specular_direction = reflect(ray.direction, hit.normal);
        ray.direction = mix(specular_direction, diffuse_direction, hit.material.roughness);
        ray.origin = hit.point;
    }

    return incomming_light;
}

@fragment
fn fs_display(
    @builtin(position) pos: vec4f,
) -> @location(0) vec4f {

    init_rng(vec2u(pos.xy));

    var scene: Scene;
    scene[0] = Sphere(
        vec3f(0.0, -100.5, 2.0),
        100.0,
        Material(
            vec3f(1.0, 1.0, 1.0),
            0.0
        )
    );
    scene[1] = Sphere(
        vec3f(0.5, 0.0, 12.0),
        4.5,
        Material(
            vec3f(0.49, 0.25, 0.88),
            0.6
        )
    );
    scene[2] = Sphere(
        vec3f(0.0, 0.0, 2.0),
        0.5,
        Material(
            vec3f(0.3, 0.7, 0.8),
            0.0
        )
    );
    scene[3] = Sphere(
        vec3f(-0.75, -0.2, 0.5),
        0.4,
        Material(
            vec3f(0.8, 0.7, 0.8),
            0.6
        )
    );

    // load previous progress
    var color: vec4f;
    if uniforms.frame_count > 1 {
        color = textureLoad(radiance_samples_old, vec2u(pos.xy), 0);
    } else {
        color = vec4f(0.0);
    }

    // save new progress and render
    let path_traced = vec4f(path_trace(pos, scene, 4), 1.0);
    color += path_traced;
    textureStore(radiance_samples_new, vec2u(pos.xy), color);

    return pow(color / f32(uniforms.frame_count), vec4f(1.0 / uniforms.gamma_correction));
    // return path_traced;
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
