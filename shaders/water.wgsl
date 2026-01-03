struct Globals {
    view_proj: mat4x4<f32>,
    morph: f32,
    _pad: vec3<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VsOut {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) flat_pos: vec3<f32>,
) -> VsOut {
    var out: VsOut;
    let t = clamp(globals.morph, 0.0, 1.0);
    let world_pos = pos * (1.0 - t) + flat_pos * t;
    out.position = globals.view_proj * vec4<f32>(world_pos, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // Soft blue-green with stronger opacity for oceans.
    return vec4<f32>(0.08, 0.32, 0.5, 0.55);
}
