use glam::Mat4;
use rand::Rng;
use std::num::NonZeroU64;
use wgpu::util::DeviceExt;

const GRID: u32 = 128;
const WORLD_SIZE: f32 = 10.0;
const HEIGHT_AMPLITUDE: f32 = 1.6;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    fn layout<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
}

pub struct Terrain {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Terrain {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        rng: &mut impl Rng,
    ) -> Self {
        let (vertices, indices) = generate_mesh(rng);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("terrain globals"),
            contents: bytemuck::bytes_of(&Globals {
                view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain bind group layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(std::mem::size_of::<Globals>() as u64),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/terrain.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
            uniform,
            bind_group,
        }
    }

    pub fn update_view(&self, queue: &wgpu::Queue, view_proj: Mat4) {
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&globals));
    }

    pub fn randomize(&mut self, queue: &wgpu::Queue, rng: &mut impl Rng) {
        let (vertices, _) = generate_mesh(rng);
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

fn generate_mesh(rng: &mut impl Rng) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity((GRID * GRID) as usize);
    for z in 0..GRID {
        for x in 0..GRID {
            let fx = (x as f32 / (GRID - 1) as f32 - 0.5) * WORLD_SIZE;
            let fz = (z as f32 / (GRID - 1) as f32 - 0.5) * WORLD_SIZE;
            let height = (rng.gen::<f32>() * 2.0 - 1.0) * HEIGHT_AMPLITUDE * 0.5
                + (rng.gen::<f32>() - 0.5) * HEIGHT_AMPLITUDE * 0.25;

            // Simple gradient: lower = darker, higher = brighter/greener.
            let t = ((height / HEIGHT_AMPLITUDE) + 0.5).clamp(0.0, 1.0);
            let color = [
                0.1 + 0.1 * t,
                0.4 + 0.4 * t,
                0.2 + 0.2 * t,
            ];

            vertices.push(Vertex {
                pos: [fx, height, fz],
                color,
            });
        }
    }

    let mut indices = Vec::with_capacity(((GRID - 1) * (GRID - 1) * 6) as usize);
    for z in 0..GRID - 1 {
        for x in 0..GRID - 1 {
            let i0 = z * GRID + x;
            let i1 = i0 + 1;
            let i2 = i0 + GRID;
            let i3 = i2 + 1;

            // Two triangles per quad
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    (vertices, indices)
}
