struct Uniforms {
    width: u32,
    height: u32,
    elapsed_seconds: f32,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

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

@fragment
fn fs_display(
    @builtin(position) pos: vec4<f32>,
) -> @location(0) vec4f {
    let sn = (sin(f32(uniforms.elapsed_seconds)) + 1.0) / 2.0;
    let cs = (cos(f32(uniforms.elapsed_seconds)) + 1.0) / 2.0;
    return vec4f(
        pos.x / f32(uniforms.width) * sn,
        pos.y / f32(uniforms.height) * cs,
        sn * cs,
        1.0
    );
}
