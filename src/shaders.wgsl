const FLOAT_MAX: f32 = 3.40282346638528859812e+38;
const EPSILON: f32 = 0.0005;
const PI: f32 = 3.14159265358;

fn is_equal_zero(a: f32) -> bool {
    return abs(a) <= EPSILON;
}

fn is_equal(a: f32, b: f32) -> bool  {
    return abs(a - b) <= EPSILON;
}

// Schlick's approximation for reflectance
fn reflectance_schlick(cosine: f32, ior: f32) -> f32 {
    var r0 = (1.0 - ior) / (1.0 + ior);
    r0 *= r0;
    var icos = 1.0 - cosine;
    return r0 + (1.0 - r0) * icos * icos * icos * icos * icos;
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
    position: vec3f,
    direction: vec3f,
    fov: f32,
    width: f32,
    focus_distance: f32,
    apeture: f32,
    diverge_strength: f32,
    max_ray_bounces: u32,
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

struct Triangle {
    vertices: array<vec3f, 3>,
    material_id: u32,
}

struct BVHNode {
    bbox_min: vec3f,
    child1: u32,
    bbox_max: vec3f,
    child2: u32,
    triangle_count: u32,
    triangle_ids: array<u32, 4>,
}

struct Scene {
    materials: array<Material, 64>,
    spheres: array<Sphere, 64>,
    triangles: array<Triangle, 256>,
    sphere_count: u32,
    triangle_count: u32,
    bvh: array<BVHNode, 96>,
}

struct Uniforms {
    camera: Camera,
    width: u32,
    height: u32,
    elapsed_seconds: f32,
    frame_count: u32,
    gamma_correction: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read> scene: Scene;
@group(0) @binding(2) var radiance_samples_old: texture_2d<f32>;
@group(0) @binding(3) var radiance_samples_new: texture_storage_2d<rgba32float, write>;

struct Ray {
    origin: vec3f,
    direction: vec3f,
}

struct HitInfo {
    distance: f32,
    point: vec3f,
    normal: vec3f,
    material_id: u32,
    front_face: bool,
}

fn sky_color(ray: Ray) -> vec3f {
    let t = 0.5 * (normalize(ray.direction).y + 1.0);
    return (1.0 - t) * vec3(1.0) + t * vec3(0.3, 0.5, 1.0);
    // return vec3f(0.0);
}

fn new_ray(pos: vec4f) -> Ray {
    let aspect_ratio = f32(uniforms.width) / f32(uniforms.height);

    var camera_right_direction = -normalize(cross(uniforms.camera.direction, vec3f(0.0, 1.0, 0.0)));
    let camera_up_direction = normalize(cross(uniforms.camera.direction, camera_right_direction));

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
    var uv = pos.xyz / vec3f(f32(uniforms.width - 1), f32(uniforms.height - 1), 1.0);
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
    var hit: HitInfo;
    hit.distance = -1.0;

    let v = ray.origin - sphere.center;
    let a = dot(ray.direction, ray.direction);
    let b = dot(v, ray.direction);
    let c = dot(v, v) - sphere.radius * sphere.radius;

    let dsc = b * b - a * c;

    hit.front_face = c > 0;

    // if the ray origin is on (or very near) the sphere surface
    // and is going outward then there must be no hit
    if is_equal_zero(c) && b >= 0 {
        return hit;
    }

    if dsc < EPSILON {
        return hit;
    }

    let sqrt_dsc = sqrt(dsc);
    let recip_a = 1.0 / a;
    let t1 = (-b - sqrt_dsc) * recip_a;
    let t2 = (-b + sqrt_dsc) * recip_a;
    hit.distance = select(t1, t2, t1 <= EPSILON);
    if hit.distance < EPSILON {
        hit.distance = -1.0;
        return hit;
    }

    hit.point = ray.origin + ray.direction * hit.distance;
    hit.normal = (hit.point - sphere.center) / sphere.radius;
    if !hit.front_face {
        hit.normal *= -1.0;
    }
    hit.material_id = sphere.material_id;

    return hit;
}

fn intersect_triangle(ray: Ray, tri: Triangle) -> HitInfo {
    var hit: HitInfo;
    hit.distance = -1.0;

    var edge0 = tri.vertices[1] - tri.vertices[0];
    var edge1 = tri.vertices[2] - tri.vertices[0];

    var normal = cross(edge0, edge1);
    var determinant = -dot(ray.direction, normal);

    hit.front_face = true;

    if is_equal_zero(determinant) {
        return hit; // ray is parallel to triangle
    }

    if determinant < 0.0 {
        // hit back face
        // let material = scene.materials[tri.material_id];
        // if material.roughness_or_ior >= 0.0 || material.volume_density >= 1.0 {
        //     return hit;
        // }

        let temp = edge0;
        edge0 = edge1;
        edge1 = temp;

        hit.front_face = false;
        normal *= -1.0;
        determinant *= -1.0;
    }

    let inv_det = 1.0 / determinant;
    let ao = ray.origin - tri.vertices[0];

    let dst = dot(ao, normal) * inv_det;
    if dst < EPSILON {
        return hit;
    }

    let dao = cross(ao, ray.direction);

    let u = dot(edge1, dao) * inv_det;
    if u < 0.0 {
        return hit;
    }

    let v = -dot(edge0, dao) * inv_det;
    if v < 0.0 {
        return hit;
    }

    let w = 1.0 - u - v;
    if w < 0.0 {
        return hit;
    }

    hit.point = ray.origin + ray.direction * dst;
    hit.normal = normalize(normal);
    hit.distance = dst;
    hit.material_id = tri.material_id;

    // if calculate_uv {
    //     let vt1 = tri.vert_texture[0];
    //     let vt2 = tri.vert_texture[1];
    //     let vt3 = tri.vert_texture[2];
    //     let coord = w * vt1 + u * vt2 + v * vt3;
    //     h.u = coord.x;
    //     h.v = coord.y;
    // }

    return hit;
}

fn intersect_aabb(ray: Ray, box_min: vec3f, box_max: vec3f) -> bool {
    let inv_dir = 1.0 / ray.direction;
    let t_min = (box_min - ray.origin) * inv_dir;
    let t_max = (box_max - ray.origin) * inv_dir;

    let t1 = vec3f(min(t_min.x, t_max.x), min(t_min.y, t_max.y), min(t_min.z, t_max.z));
    let t2 = vec3f(max(t_min.x, t_max.x), max(t_min.y, t_max.y), max(t_min.z, t_max.z));

    let t_near = max(max(t1.x, t1.y), t1.z);
    let t_far = min(min(t2.x, t2.y), t2.z);

    return t_near <= t_far;
}

fn intersect_bvh(ray: Ray) -> HitInfo {
    var hit: HitInfo;
    hit.distance = FLOAT_MAX;
    var stack: array<u32, 64>;
    var stack_ptr = 1u;
    stack[0] = 0;

    while stack_ptr > 0u {
        stack_ptr -= 1u;
        let node_index = stack[stack_ptr];
        let node = scene.bvh[node_index];

        if !intersect_aabb(ray, node.bbox_min, node.bbox_max) {
            continue;
        }

        if node.triangle_count != 0u {
            // leaf node: test all triangles
            for (var i = 0u; i < node.triangle_count; i += 1u) {
                let tri_id = node.triangle_ids[i];
                let tri = scene.triangles[tri_id];
                let h = intersect_triangle(ray, tri);
                if h.distance >= EPSILON && h.distance < hit.distance {
                    hit = h;
                }
            }
        } else {
            // internal node: push children
            stack[stack_ptr] = node.child1;
            stack_ptr += 1u;
            if stack_ptr >= 64 {
                return hit;
            }

            stack[stack_ptr] = node.child2;
            stack_ptr += 1u;
            if stack_ptr >= 64 {
                return hit;
            }
        }
    }

    if hit.distance == FLOAT_MAX {
        hit.distance = -1.0;
    }

    return hit;
}

fn get_ray_collision(ray: Ray) -> HitInfo {
    var closest_hit: HitInfo;
    closest_hit.distance = FLOAT_MAX;

    // sphere
    for(var i = 0u; i < scene.sphere_count; i += 1u) {
        let hit = intersect_sphere(ray, scene.spheres[i]);
        if hit.distance >= EPSILON && hit.distance < closest_hit.distance {
            closest_hit = hit;
        }
    }

    // use linear search if tris count is low
    if scene.triangle_count < 16 {
        for(var i = 0u; i < scene.triangle_count; i += 1u) {
            let hit = intersect_triangle(ray, scene.triangles[i]);
            if hit.distance >= EPSILON && hit.distance < closest_hit.distance {
                closest_hit = hit;
            }
        }
    } else {
        let bvh_hit = intersect_bvh(ray);
        if bvh_hit.distance >= EPSILON && bvh_hit.distance < closest_hit.distance {
            closest_hit = bvh_hit;
        }
    }

    if closest_hit.distance == FLOAT_MAX {
        closest_hit.distance = -1.0;
    }
    return closest_hit;
}

fn path_trace(ray_pos: vec4f, initial_ray_color: vec3f) -> vec3f {
    var incomming_light = vec3f(0.0);
    var ray_color = initial_ray_color;

    var ray = new_ray(ray_pos);

    var surrounding_volume_density = 0.0;
    var surrounding_volume_radiance = vec3f(0.0);

    // // check surrounding
    // for(var i = 0u; i < scene.sphere_count; i += 1u) {
    //     let sphere = scene.spheres[i];
    //     let d = ray.origin - sphere.center;
    //     if dot(d, d) < sphere.radius * sphere.radius {
    //         let material = scene.materials[sphere.material_id];
    //         surrounding_volume_density += material.volume_density;
    //         surrounding_volume_radiance += material.emission_strength * material.color;
    //     }
    // }

    var bounces = 0u;
    while bounces < uniforms.camera.max_ray_bounces {
        let hit = get_ray_collision(ray);

        if hit.distance < EPSILON {
            incomming_light += ray_color * sky_color(ray);
            break;
        }

        let material = scene.materials[hit.material_id];
        let new_ray_color = ray_color * material.color;
        if new_ray_color.x == new_ray_color.y && new_ray_color.x == new_ray_color.z && new_ray_color.x == 0.0 {
            break;
        }

        if surrounding_volume_density > 0.0 {
            let scattering_distance = -log(rand()) / surrounding_volume_density;

            if scattering_distance < hit.distance {
                // hit the particle
                let transmittance = exp(-surrounding_volume_density * scattering_distance);
                let radiance = surrounding_volume_radiance * (1.0 - transmittance);
                incomming_light += ray_color * radiance;
                ray_color *= transmittance;
                ray.origin += ray.direction * scattering_distance;
                ray.direction = rand_sphere();
                bounces += 1;
                continue;
            }
        }

        if material.volume_density < 1.0 {
            if !hit.front_face {
                surrounding_volume_density -= material.volume_density;
                surrounding_volume_radiance -= material.emission_strength * material.color;
                if is_equal_zero(surrounding_volume_density) {
                    surrounding_volume_density = 0.0;
                    surrounding_volume_radiance = vec3f(0.0);
                }
            } else {
                surrounding_volume_density += material.volume_density;
                surrounding_volume_radiance += material.emission_strength * material.color;
            }
            ray.origin = hit.point + ray.direction * EPSILON;
            // recalculate again to account for smoke
            continue;
        }

        if material.roughness_or_ior > 0.0 {
            // calculate scattering direction
            let diffuse_direction = normalize(hit.normal + (1.0 - EPSILON) * rand_sphere());
            let specular_direction = reflect(ray.direction, hit.normal);
            ray.direction = mix(specular_direction, diffuse_direction, material.roughness_or_ior);
        } else {
            let cos_theta = abs(dot(ray.direction, hit.normal));

            var base_ior = -material.roughness_or_ior;
            let ior = select(base_ior, 1.0 / base_ior, hit.front_face);
            let cannot_refract = ior * ior * (1.0 - cos_theta * cos_theta) > 1.0;

            if cannot_refract || reflectance_schlick(cos_theta, ior) > rand() {
                ray.direction = reflect(ray.direction, hit.normal);
            } else {
                ray.direction = refract(ray.direction, hit.normal, ior);
            }
        }
        ray.origin = hit.point + ray.direction * EPSILON;

        // ray_color *= hit.normal * 0.5 + vec3f(0.5);
        ray_color = new_ray_color;
        incomming_light += ray_color * material.emission_strength;

        bounces += 1;
    }

    return incomming_light;
}

@fragment
fn fs_display(
    @builtin(position) pos: vec4f,
) -> @location(0) vec4f {


    init_rng(vec2u(pos.xy));

    // load previous progress
    var color: vec4f;
    if uniforms.frame_count > 1 {
        color = textureLoad(radiance_samples_old, vec2u(pos.xy), 0);
    } else {
        color = vec4f(0.0);
    }

    // save new progress and render
    let path_traced = vec4f(path_trace(pos, vec3f(1.0)), 1.0);
    color += path_traced;
    textureStore(radiance_samples_new, vec2u(pos.xy), color);

    return pow(color / f32(uniforms.frame_count), vec4f(1.0 / uniforms.gamma_correction));
    // return pow(path_traced, vec4f(1.0 / uniforms.gamma_correction));
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
