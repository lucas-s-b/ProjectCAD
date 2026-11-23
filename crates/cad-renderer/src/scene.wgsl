// clip: keep points with dot(clip.xyz, p) <= clip.w; (0, 0, 0, 1) keeps everything.
struct Camera { view_proj: mat4x4<f32>, clip: vec4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world: vec3<f32>,
};

fn vertex_out(pos: vec3<f32>, normal: vec3<f32>, color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.pos = camera.view_proj * vec4<f32>(pos, 1.0);
    // Lines carry a zero normal: pull them toward the camera so edges win over coplanar faces.
    if (dot(normal, normal) < 0.01) { out.pos.z -= 0.0002 * out.pos.w; }
    out.normal = normal;
    out.color = color;
    out.world = pos;
    return out;
}

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec4<f32>) -> VsOut {
    return vertex_out(pos, normal, color);
}

// Instanced meshes: the model matrix places the mesh; triangles take the instance colour, lines (zero normal) keep theirs.
@vertex
fn vs_instanced(
    @location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec4<f32>,
    @location(3) m0: vec4<f32>, @location(4) m1: vec4<f32>, @location(5) m2: vec4<f32>, @location(6) m3: vec4<f32>,
    @location(7) tint: vec4<f32>,
) -> VsOut {
    let model = mat4x4<f32>(m0, m1, m2, m3);
    let world = (model * vec4<f32>(pos, 1.0)).xyz;
    let n = (model * vec4<f32>(normal, 0.0)).xyz;
    return vertex_out(world, n, select(color * tint, color, dot(normal, normal) < 0.01));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (in.color.a > 0.5 && dot(camera.clip.xyz, in.world) > camera.clip.w) { discard; }
    if (dot(in.normal, in.normal) < 0.01) { return vec4<f32>(in.color.rgb, 1.0); }
    let l = normalize(vec3<f32>(0.4, 0.3, 0.85));
    let shade = 0.35 + 0.65 * abs(dot(normalize(in.normal), l));
    return vec4<f32>(in.color.rgb * shade, 1.0);
}
