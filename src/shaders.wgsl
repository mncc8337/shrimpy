const FLOAT_MAX: f32 = 3.40282346638528859812e+38;
const EPSILON: f32 = 0.000001;
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

fn random_point_in_circle() -> vec2f {
    let angle = rand() * 2 * PI;
    let point_on_circle = vec2f(cos(angle), sin(angle));
    return point_on_circle * sqrt(rand());
}

struct Uniforms {
    width: u32,
    height: u32,
    elapsed_seconds: f32,
    frame_count: u32,

    // camera_position: vec3f,
    // camera_direction: vec3f,
    camera_fov: f32,
    camera_apeture: f32,
    camera_diverge_strength: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var screen_texture_old: texture_2d<f32>;
@group(0) @binding(2) var screen_texture_new: texture_storage_2d<rgba32float, write>;

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
}

struct Sphere {
    center: vec3f,
    radius: f32,
}

fn sky_color(ray: Ray) -> vec3f {
    let t = 0.5 * (normalize(ray.direction).y + 1.0);
    return (1.0 - t) * vec3(1.0) + t * vec3(0.3, 0.5, 1.0);
}

fn intersect_sphere(ray: Ray, sphere: Sphere) -> HitInfo {
    var hit = HitInfo(
        false,
        vec3f(0.0),
        vec3f(0.0),
        0.0,
        false
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

    hit.inside_object = c < 0.0;

    let sqrt_dsc = sqrt(dsc);
    let recip_a = 1.0 / a;
    let t1 = (-b - sqrt_dsc) * recip_a;
    let t2 = (-b + sqrt_dsc) * recip_a;
    hit.distance = select(t2, t1, t1 > 0.0);
    if hit.distance < 0.0 {
        return hit;
    }

    hit.point = ray.origin + ray.direction * hit.distance;
    hit.normal = (hit.point - sphere.center) / sphere.radius;
    if hit.inside_object {
        hit.normal *= -1;
    }

    hit.did_hit = true;
    return hit;
}

const OBJECT_COUNT: u32 = 2;
alias Scene = array<Sphere, OBJECT_COUNT>;
var<private> scene: Scene = Scene(
    Sphere(vec3f(0.5, 0., 2.), 0.5),
    Sphere(vec3f(0., -100.5, 2.), 100.),
);

@fragment
fn fs_display(
    @builtin(position) pos: vec4<f32>,
) -> @location(0) vec4f {
    init_rng(vec2u(pos.xy));

    let aspect_ratio = f32(uniforms.width) / f32(uniforms.height);

    let camera_position = vec3f(0.0);
    let camera_direction = vec3f(0.0, 0.0, 1.0);

    let camera_right_direction = vec3f(camera_direction.z, 0.0, -camera_direction.x);
    let camera_up_direction = cross(camera_direction, camera_right_direction);

    // offset ray origin for defocusing effect
    let defocus_jitter = vec3f(random_point_in_circle() * uniforms.camera_apeture * 0.5, 0.0);
    var ray_origin = camera_position;
    ray_origin += camera_up_direction * defocus_jitter.y + camera_right_direction * defocus_jitter.x;

    // random jitter for anti-aliasing
    let jitter = random_point_in_circle() * uniforms.camera_diverge_strength;
    var uv = pos.xyz / vec3f(f32(uniforms.width - 1), f32(uniforms.height - 1), 0.0);
    uv = (2.0 * uv - vec3f(1.0)) * vec3f(aspect_ratio, -1.0, 0.0);
    uv = camera_up_direction * uv.y + camera_right_direction * uv.x;

    var ray = Ray (
        ray_origin,
        uv + camera_direction / tan(uniforms.camera_fov * 0.5),
    );

    // scene[0].center.x = sin(f32(uniforms.elapsed_seconds));

    var closest_hit = HitInfo(
        false,
        vec3f(0.0),
        vec3f(0.0),
        FLOAT_MAX,
        false
    );
    for(var i = 0u; i < OBJECT_COUNT; i += 1u) {
        let hit = intersect_sphere(ray, scene[i]);
        if !hit.did_hit {
            continue;
        }

        if hit.distance < closest_hit.distance {
            closest_hit= hit;
        }
    }

    var color: vec4f;
    if uniforms.frame_count > 1 {
        color = textureLoad(screen_texture_old, vec2u(pos.xy), 0);
        color *= f32(uniforms.frame_count - 1);
    } else {
        color = vec4f(0.0);
    }

    if closest_hit.distance < FLOAT_MAX {
        color += vec4f(closest_hit.normal * 0.5 + vec3f(0.5), 1.0);
    } else {
        color += vec4f(sky_color(ray), 1.0);
    }
    color /= f32(uniforms.frame_count);

    textureStore(screen_texture_new, vec2u(pos.xy), color);
    return color;
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
