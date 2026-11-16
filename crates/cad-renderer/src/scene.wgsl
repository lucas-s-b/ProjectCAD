struct Camera { view_proj: mat4x4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.pos = camera.view_proj * vec4<f32>(pos, 1.0);
    // Lines carry a zero normal: pull them toward the camera so edges win over coplanar faces.
    if (dot(normal, normal) < 0.01) { out.pos.z -= 0.0002 * out.pos.w; }
    out.normal = normal;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (dot(in.normal, in.normal) < 0.01) { return in.color; }
    let l = normalize(vec3<f32>(0.4, 0.3, 0.85));
    let shade = 0.35 + 0.65 * abs(dot(normalize(in.normal), l));
    return vec4<f32>(in.color.rgb * shade, in.color.a);
}
