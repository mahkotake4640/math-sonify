// Phase portrait 3D trail shader.
//
// Vertex shader: transforms 3D trajectory points into clip space.
// Fragment shader: outputs the trail colour with alpha fade + bloom prep.

struct Uniforms {
    view_proj: mat4x4<f32>,
    trail_color: vec4<f32>,
    point_size: f32,
    time: f32,
    bloom_threshold: f32,
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_pos: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.color = in.color * uniforms.trail_color;
    out.world_pos = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = in.color;

    // Additive glow: amplify bright fragments for bloom.
    let luminance = dot(c.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let glow_factor = select(
        1.0,
        1.0 + (luminance - uniforms.bloom_threshold) * 2.0,
        luminance > uniforms.bloom_threshold
    );

    let rgb = c.rgb * glow_factor;
    return vec4<f32>(rgb, c.a);
}
