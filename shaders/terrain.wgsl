struct Globals {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> globals: Globals;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
) -> VsOut {
    var out: VsOut;
    out.position = globals.view_proj * vec4<f32>(pos, 1.0);
    out.normal = normal;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.4, 0.9, 0.2));
    let n = normalize(in.normal);
    let ndl = clamp(dot(n, light_dir), 0.0, 1.0);
    let ambient = 0.45;
    let diffuse = ndl * 0.55;
    let shading = ambient + diffuse;
    return vec4<f32>(in.color * shading, 1.0);
}
