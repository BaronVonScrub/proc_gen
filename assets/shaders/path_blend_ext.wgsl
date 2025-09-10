#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}
#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

struct PathBlendParams {
    fade_radius: f32,
    // Global thickness multiplier: distances are divided by this before falloff.
    // 1.0 = no change; >1.0 = thicker; <1.0 = thinner.
    thickness_scale: f32,
    // Inner core width where the path is fully near (no falloff). Falloff begins beyond this.
    base_width: f32,
    min_blend: f32,
    max_blend: f32,
    near_metallic: f32,
    near_roughness: f32,
    flags: vec4<u32>, // x=falloff_mode, y=invert, z=segment_count
    near_base_color: vec4<f32>,
    segments: array<vec4<f32>, 256>, // (ax, az, bx, bz)
};

@group(2) @binding(100)
var<uniform> path_blend: PathBlendParams;

// Optional near albedo texture; presence encoded by flags.w bit0
@group(2) @binding(101)
var near_albedo_tex: texture_2d<f32>;
@group(2) @binding(102)
var near_albedo_smp: sampler;

// Optional near metallic-roughness texture; presence encoded by flags.w bit1
@group(2) @binding(103)
var near_mr_tex: texture_2d<f32>;
@group(2) @binding(104)
var near_mr_smp: sampler;

// Optional near ambient occlusion texture; presence encoded by flags.w bit2
@group(2) @binding(105)
var near_ao_tex: texture_2d<f32>;
@group(2) @binding(106)
var near_ao_smp: sampler;


fn distance_point_to_segment_2d(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let ab_len2 = max(dot(ab, ab), 1e-6);
    let t = clamp(dot(ap, ab) / ab_len2, 0.0, 1.0);
    let closest = a + t * ab;
    return length(p - closest);
}

fn compute_near_weight(d: f32) -> f32 {
    let mode = path_blend.flags.x;
    let inv = path_blend.flags.y;
    let r = max(path_blend.fade_radius, 1e-6);
    let s = max(path_blend.thickness_scale, 1e-6);
    // Subtract the core width first so we have an inner flat region at w=1
    let core = max(path_blend.base_width, 0.0);
    let d_core = max(d - core, 0.0);
    let ds = d_core / s;
    var w: f32;
    if (mode == 0u) { // smoothstep near = 1 at d=0, 0 at d=r
        w = 1.0 - smoothstep(0.0, r, ds);
    } else if (mode == 1u) { // inverse squared
        let k = 1.0 / (r * r);
        w = 1.0 / (1.0 + k * ds * ds);
    } else { // linear
        w = clamp(1.0 - ds / r, 0.0, 1.0);
    }
    if (inv != 0u) {
        w = 1.0 - w;
    }
    // clamp to min/max blend
    return clamp(mix(path_blend.min_blend, path_blend.max_blend, w), 0.0, 1.0);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // Generate PBR input from StandardMaterial
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // Compute world XZ distance to nearest path segment
    let p = vec2<f32>(pbr_input.world_position.x, pbr_input.world_position.z);
    let count = path_blend.flags.z;
    var min_d = 1e9;
    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let seg = path_blend.segments[i];
        let a = vec2<f32>(seg.x, seg.y);
        let b = vec2<f32>(seg.z, seg.w);
        let d = distance_point_to_segment_2d(p, a, b);
        min_d = min(min_d, d);
    }
    let w = compute_near_weight(min_d);

    // Near color: optionally sample a texture when provided (flags.w bit0), else use uniform near_base_color
    let near_has_albedo: bool = (path_blend.flags.w & 0x1u) != 0u;
    let near_has_mr: bool = (path_blend.flags.w & 0x2u) != 0u;
    let near_has_ao: bool = (path_blend.flags.w & 0x4u) != 0u;
    var near_col = path_blend.near_base_color;
    if (near_has_albedo) {
        near_col = textureSample(near_albedo_tex, near_albedo_smp, in.uv);
    }
    pbr_input.material.base_color = mix(pbr_input.material.base_color, near_col, w);
    // Blend metallic/roughness
    if (near_has_mr) {
        let mr = textureSample(near_mr_tex, near_mr_smp, in.uv);
        // glTF MR convention: roughness in G, metallic in B
        let near_roughness = mr.g;
        let near_metallic = mr.b;
        pbr_input.material.metallic = mix(pbr_input.material.metallic, near_metallic, w);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, near_roughness, w);
    } else {
        pbr_input.material.metallic = mix(pbr_input.material.metallic, path_blend.near_metallic, w);
        pbr_input.material.perceptual_roughness = mix(pbr_input.material.perceptual_roughness, path_blend.near_roughness, w);
    }
    // Optional: AO blending if accessible in material (commented out unless validated)
    // if (near_has_ao) {
    //     let ao = textureSample(near_ao_tex, near_ao_smp, in.uv).r;
    //     pbr_input.material.occlusion = mix(pbr_input.material.occlusion, ao, w);
    // }

    // Alpha discard and lighting
    pbr_input.material.base_color = alpha_discard(pbr_input.material, pbr_input.material.base_color);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif
    return out;
}
