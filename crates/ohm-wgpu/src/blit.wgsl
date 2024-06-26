@group(0) @binding(0)
var texture: texture_2d<f32>;

@group(0) @binding(1)
var texture_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,    
    @location(0) tex: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_id: u32) -> VertexOutput {
    var out: VertexOutput;

    let x = select(-1.0, 3.0, vertex_id == 1u);
    let y = select(-1.0, 3.0, vertex_id == 2u);
    out.clip_pos = vec4(x, y, 0.0, 1.0);
    out.tex.x = out.clip_pos.x * 0.5 + 0.5;
    out.tex.y = 0.5 - out.clip_pos.y * 0.5;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(texture, texture_sampler, in.tex);
}
