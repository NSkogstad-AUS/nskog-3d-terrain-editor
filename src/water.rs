use glam::{Mat4, Vec3};
use std::num::NonZeroU64;
use wgpu::util::DeviceExt;

use crate::terrain::WORLD_RADIUS;

const MAP_WIDTH: f32 = WORLD_RADIUS * std::f32::consts::TAU;
const MAP_HEIGHT: f32 = WORLD_RADIUS * std::f32::consts::PI;
const FLAT_WATER_OFFSET: f32 = 1.2;
const GLOBE_WATER_OFFSET: f32 = 0.6;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    flat_pos: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    morph: [f32; 4],
}

pub struct Water {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    index_count: u32,
}

impl Water {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, height: f32) -> Self {
        let (vertices, indices) = generate_sphere(WORLD_RADIUS + height - GLOBE_WATER_OFFSET, height);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("water globals"),
            contents: bytemuck::bytes_of(&Globals {
                view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                morph: [0.0; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("water bind group layout"),
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
            label: Some("water bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("water shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/water.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("water pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("water pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3],
                }],
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform,
            bind_group,
            index_count: indices.len() as u32,
        }
    }

    pub fn update_view(
        &self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        morph: f32,
        rotation: f32,
    ) {
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            morph: [
                morph.clamp(0.0, 1.0),
                rotation,
                MAP_WIDTH,
                MAP_HEIGHT,
            ],
        };
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&globals));
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }
}

const WATER_RES: u32 = 128;
const WATER_LON: u32 = WATER_RES + 1;

fn generate_sphere(radius: f32, height: f32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity((WATER_RES * WATER_LON) as usize);
    for z in 0..WATER_RES {
        let v = z as f32 / (WATER_RES - 1) as f32;
        let lat = v * std::f32::consts::PI;
        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        for x in 0..WATER_LON {
            let u = x as f32 / (WATER_LON - 1) as f32;
            let lon = u * std::f32::consts::TAU;
            let dir = Vec3::new(lon.cos() * sin_lat, cos_lat, lon.sin() * sin_lat);
            vertices.push(Vertex {
                pos: (dir * radius).into(),
                flat_pos: [u, v, height - FLAT_WATER_OFFSET],
            });
        }
    }

    let mut indices = Vec::with_capacity(((WATER_RES - 1) * (WATER_LON - 1) * 6) as usize);
    for z in 0..WATER_RES - 1 {
        for x in 0..WATER_LON - 1 {
            let i0 = z * WATER_LON + x;
            let i1 = i0 + 1;
            let i2 = i0 + WATER_LON;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
        }
    }

    (vertices, indices)
}
