struct Globals {
    view_proj: mat4x4<f32>,
    morph: vec4<f32>,
};

const INV_TAU: f32 = 0.15915494;

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
    let t = clamp(globals.morph.x, 0.0, 1.0);
    let rot = globals.morph.y;
    let rot_blend = rot * t;
    let c = cos(rot_blend);
    let s = sin(rot_blend);
    let globe_pos = vec3<f32>(pos.x * c + pos.z * s, pos.y, -pos.x * s + pos.z * c);
    let map_width = globals.morph.z;
    let map_height = globals.morph.w;
    let u = fract(flat_pos.x + rot * INV_TAU);
    let v = clamp(flat_pos.y, 0.0, 1.0);
    let height = flat_pos.z;
    let flat_x = (u - 0.5) * map_width;
    let flat_z = (0.5 - v) * map_height;
    let flat_world = vec3<f32>(flat_x, height, flat_z);
    let world_pos = globe_pos * (1.0 - t) + flat_world * t;
    out.position = globals.view_proj * vec4<f32>(world_pos, 1.0);
    return out;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    // Soft blue-green with stronger opacity for oceans.
    return vec4<f32>(0.08, 0.32, 0.5, 0.55);
}
