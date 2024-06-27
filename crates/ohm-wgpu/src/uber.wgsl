struct Globals {
    resolution: vec2<f32>,    
}

@group(0) @binding(0)
var<uniform> globals: Globals;

struct RectInstance {
    corner_radii: vec4<f32>,
    border_color: vec4<f32>,
    shadow_color: vec4<f32>,
    shadow_offset: vec2<f32>,
    size: vec2<f32>,
    border_width: f32,
    shadow_blur_radius: f32,
    shadow_spread_radius: f32,
}

struct RectInstances {
    arr: array<RectInstance, 128>,
}

@group(0) @binding(1)
var<uniform> rect_instances: RectInstances;

@group(0) @binding(2)
var texture: texture_2d<f32>;

@group(0) @binding(3)
var texture_sampler: sampler;

struct VertexInput {
    @location(0) pos: vec2<f32>,    
    @location(1) local_pos: vec2<f32>,    
    @location(2) tex: vec2<f32>,    
    @location(3) color: vec4<f32>,
    @location(4) instance_id: u32,
}

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,    
    @location(0) pos: vec2<f32>,
    @location(1) tex: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) instance_id: u32,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    out.clip_pos = vec4(
        in.pos.x / globals.resolution.x * 2.0 - 1.0,
        1.0 - in.pos.y / globals.resolution.y * 2.0,
        0.0,
        1.0
    );

    out.pos = in.local_pos;
    out.tex = in.tex;
    out.color = in.color;
    out.instance_id = in.instance_id;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var base_color = textureSample(texture, texture_sampler, in.tex);

    if in.instance_id == 4294967294u {
        return in.color * base_color.r;
    }

    if in.instance_id == 4294967295u {
        return in.color * base_color;
    }

    let rect = rect_instances.arr[in.instance_id];

    let pos = in.pos - rect.size / 2.0;

    let dist = sdf_rounded_rect(pos, rect.size / 2.0, rect.corner_radii);
    let dist_change = fwidth(dist) * 0.5;
    let mask = smoothstep(dist_change, -dist_change, dist);

    var color = in.color * base_color;
    if rect.border_width > 0.001 {
        let border_mask = smoothstep(dist_change, -dist_change, dist + rect.border_width);
        color = mix(rect.border_color, color, border_mask);
    }

    if rect.shadow_color.a > 0.001 {
        let size = rect.size / 2.0 + rect.shadow_spread_radius;
        var radii = rect.corner_radii;
        radii += sign(radii) * rect.shadow_spread_radius;
        
        var shadow = 0.0;
        if rect.shadow_blur_radius < 1.0 {
            let shadow_dist = sdf_rounded_rect(pos - rect.shadow_offset, size, radii);
            let shadow_dist_change = fwidth(shadow_dist) * 0.5;
            shadow = smoothstep(shadow_dist_change, -shadow_dist_change, shadow_dist);
        } else {
            let sigma = 0.5 * rect.shadow_blur_radius;
            shadow = sdf_shadow(pos - rect.shadow_offset, size, radii, sigma);
        }

        color = mix(shadow * rect.shadow_color, color, mask);
    } else {
        color *= mask;
    }

    return color;
}

fn sdf_rounded_rect(p: vec2<f32>, b: vec2<f32>, radius: vec4<f32>) -> f32 {
    let rr = select(radius.xw, radius.yz, p.x > 0.0);
    let r = select(rr.x, rr.y, p.y > 0.0);
    let q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

fn sdf_shadow(p: vec2<f32>, b: vec2<f32>, radius: vec4<f32>, s: f32) -> f32 {
    let rr = select(radius.xw, radius.yz, p.x > 0.0);
    let r = select(rr.x, rr.y, p.y > 0.0);
    let r0 = length(vec2(r, s * 1.15));
    let r1 = length(vec2(r, s * 2.0));

    let exponent = 2.0 * r1 / r0;
    let s_inv = 1.0 / max(s, 1e-6);

    let s_inv_b = s_inv * b;
    let s_inv_b_2 = s_inv_b * s_inv_b;
    let delta = 0.0 * 1.25 * s * (exp(-s_inv_b_2.x) - exp(-s_inv_b_2.y));

    let w = 2.0 * b.x + min(delta, 0.0);
    let h = 2.0 * b.y + min(delta, 0.0);

    let x0 = abs(p.x) - 0.5 * w + r1;
    let x1 = max(x0, 0.0);

    let y0 = abs(p.y) - 0.5 * h + r1;
    let y1 = max(y0, 0.0);

    let d_pos = pow(pow(x1, exponent) + pow(y1, exponent), 1.0 / exponent);
    let d_neg = min(max(x0, y0), 0.0);
    let d = d_pos + d_neg - r1;

    return 0.5 - erf(1.0 * (s_inv * d - 0.5)) * 0.5;
}

fn erf(v: f32) -> f32 {
    let x = v * 1.1283791670954755;
    let xx = x * x;
    let y = x + (0.24295 + (0.03395 + 0.0104 * xx) * xx) * (x * xx);
    return y * inverseSqrt(1.0 + y * y);
}

