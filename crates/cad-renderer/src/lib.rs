use wgpu::util::DeviceExt;

mod instancing;
pub use instancing::{Batch, Draw, Instance, MeshData, batch};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

pub const SHADER: &str = include_str!("scene.wgsl");

pub struct SceneRenderer {
    tri_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    camera_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    tris: Option<(wgpu::Buffer, u32)>,
    lines: Option<(wgpu::Buffer, u32)>,
    tri_instanced: wgpu::RenderPipeline,
    line_instanced: wgpu::RenderPipeline,
    meshes: std::collections::HashMap<u64, GpuMesh>,
    instances: Option<wgpu::Buffer>,
    batches: Vec<Batch>,
}

/// A body mesh on the GPU, shared by all its instances.
struct GpuMesh {
    tris: Option<(wgpu::Buffer, u32)>,
    lines: Option<(wgpu::Buffer, u32)>,
}

fn upload(device: &wgpu::Device, v: &[Vertex]) -> Option<(wgpu::Buffer, u32)> {
    (!v.is_empty()).then(|| {
        let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_vertices"),
            contents: bytemuck::cast_slice(v),
            usage: wgpu::BufferUsages::VERTEX,
        });
        (buf, v.len() as u32)
    })
}

impl SceneRenderer {
    pub fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
        samples: u32,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scene_camera"),
            contents: bytemuck::bytes_of(&[[0.0f32; 4]; 5]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_bg"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() }],
        });

        let attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];
        let inst_attrs = wgpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4];
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attrs,
        };
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &inst_attrs,
        };
        let make = |topology: wgpu::PrimitiveTopology, compare: wgpu::CompareFunction, instanced: bool| {
            let buffers: &[Option<wgpu::VertexBufferLayout<'_>>] =
                if instanced { &[Some(vertex_layout.clone()), Some(instance_layout.clone())] } else { &[Some(vertex_layout.clone())] };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(if instanced { "vs_instanced" } else { "vs_main" }),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState { topology, cull_mode: None, ..Default::default() },
                depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(compare),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState { count: samples.max(1), mask: !0, alpha_to_coverage_enabled: false },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        Self {
            tri_pipeline: make(wgpu::PrimitiveTopology::TriangleList, wgpu::CompareFunction::Less, false),
            line_pipeline: make(wgpu::PrimitiveTopology::LineList, wgpu::CompareFunction::LessEqual, false),
            tri_instanced: make(wgpu::PrimitiveTopology::TriangleList, wgpu::CompareFunction::Less, true),
            line_instanced: make(wgpu::PrimitiveTopology::LineList, wgpu::CompareFunction::LessEqual, true),
            camera_buffer,
            bind_group,
            tris: None,
            lines: None,
            meshes: std::collections::HashMap::new(),
            instances: None,
            batches: Vec::new(),
        }
    }

    pub fn set_geometry(&mut self, device: &wgpu::Device, tris: &[Vertex], lines: &[Vertex]) {
        self.tris = upload(device, tris);
        self.lines = upload(device, lines);
    }

    /// Keeps on the GPU exactly the meshes listed (uploading keys not seen before; a key's data never changes) and
    /// replaces the instances with `draws`. Draws whose mesh is not listed are skipped.
    pub fn set_instances(&mut self, device: &wgpu::Device, meshes: &[(u64, std::sync::Arc<MeshData>)], draws: &[Draw]) {
        self.meshes.retain(|k, _| meshes.iter().any(|(m, _)| m == k));
        for (k, m) in meshes {
            self.meshes.entry(*k).or_insert_with(|| GpuMesh { tris: upload(device, &m.tris), lines: upload(device, &m.lines) });
        }
        let (instances, batches) = batch(draws);
        self.instances = (!instances.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("scene_instances"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });
        self.batches = batches;
    }

    /// `clip` is the section plane: points with `dot(clip.xyz, p) > clip.w` are dropped from clippable geometry (vertex alpha 1).
    pub fn set_camera(&self, queue: &wgpu::Queue, view_proj: [[f32; 4]; 4], clip: [f32; 4]) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&view_proj));
        queue.write_buffer(&self.camera_buffer, 64, bytemuck::bytes_of(&clip));
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_bind_group(0, &self.bind_group, &[]);
        let kinds = [(&self.tri_pipeline, &self.tris, &self.tri_instanced, true), (&self.line_pipeline, &self.lines, &self.line_instanced, false)];
        for (pipeline, geometry, instanced, triangles) in kinds {
            if let Some((buf, n)) = geometry {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..*n, 0..1);
            }
            self.draw_instanced(pass, instanced, triangles);
        }
    }

    fn draw_instanced(&self, pass: &mut wgpu::RenderPass<'_>, pipeline: &wgpu::RenderPipeline, triangles: bool) {
        let Some(inst) = &self.instances else { return };
        pass.set_pipeline(pipeline);
        pass.set_vertex_buffer(1, inst.slice(..));
        for b in self.batches.iter().filter(|b| b.faces || !triangles) {
            let mesh = self.meshes.get(&b.mesh);
            if let Some((buf, n)) = mesh.and_then(|m| if triangles { m.tris.as_ref() } else { m.lines.as_ref() }) {
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..*n, b.range.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_parses_and_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(SHADER).expect("wgsl parse");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("wgsl validate");
    }

    #[test]
    fn vertex_is_40_bytes() {
        assert_eq!(std::mem::size_of::<Vertex>(), 40);
    }
}
