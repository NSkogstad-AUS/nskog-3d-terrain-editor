use glam::{Mat4, Vec3};
use rand::Rng;
use std::num::NonZeroU64;
use wgpu::util::DeviceExt;

pub const GRID: u32 = 256;
pub const WORLD_RADIUS: f32 = 80.0;
pub const HEIGHT_AMPLITUDE: f32 = 10.0;
pub const WATER_LEVEL: f32 = 0.0;
const CONTINENT_FREQ: f32 = 0.8;
const DETAIL_FREQ: f32 = 5.5;
const SEA_THRESHOLD: f32 = 0.18;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];

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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
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
    let continent_seed = rng.gen::<u32>();
    let detail_seed = rng.gen::<u32>();

    let mut heights = vec![0.0f32; (GRID * GRID) as usize];
    let mut positions = vec![Vec3::ZERO; (GRID * GRID) as usize];
    for z in 0..GRID {
        let lat = z as f32 / (GRID - 1) as f32 * std::f32::consts::PI;
        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        for x in 0..GRID {
            let lon = x as f32 / GRID as f32 * std::f32::consts::TAU;
            let dir = Vec3::new(lon.cos() * sin_lat, cos_lat, lon.sin() * sin_lat);
            let height = height_for_dir(dir, continent_seed, detail_seed);
            let idx = (z * GRID + x) as usize;
            heights[idx] = height;
            positions[idx] = dir * (WORLD_RADIUS + height);
        }
    }

    let mut normals = vec![Vec3::ZERO; positions.len()];
    for z in 0..GRID {
        for x in 0..GRID {
            let idx = (z * GRID + x) as usize;
            if z == 0 || z == GRID - 1 {
                normals[idx] = positions[idx].normalize_or_zero();
                continue;
            }

            let left = positions[(z * GRID + (x + GRID - 1) % GRID) as usize];
            let right = positions[(z * GRID + (x + 1) % GRID) as usize];
            let down = positions[((z - 1) * GRID + x) as usize];
            let up = positions[((z + 1) * GRID + x) as usize];
            let tangent = right - left;
            let bitangent = up - down;
            let normal = tangent.cross(bitangent).normalize_or_zero();
            normals[idx] = if normal.length_squared() > 0.0 {
                normal
            } else {
                positions[idx].normalize_or_zero()
            };
        }
    }

    let mut vertices = Vec::with_capacity((GRID * GRID) as usize);
    for idx in 0..positions.len() {
        let height = heights[idx];
        let color = color_from_height(height);
        vertices.push(Vertex {
            pos: positions[idx].into(),
            normal: normals[idx].into(),
            color,
        });
    }

    let mut indices = Vec::with_capacity((GRID * (GRID - 1) * 6) as usize);
    for z in 0..GRID - 1 {
        for x in 0..GRID {
            let x_next = (x + 1) % GRID;
            let i0 = z * GRID + x;
            let i1 = z * GRID + x_next;
            let i2 = (z + 1) * GRID + x;
            let i3 = (z + 1) * GRID + x_next;
            indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    (vertices, indices)
}

fn height_for_dir(dir: Vec3, continent_seed: u32, detail_seed: u32) -> f32 {
    let continent = fbm(dir * CONTINENT_FREQ, continent_seed, 4, 2.05, 0.5);
    let base = continent - SEA_THRESHOLD;
    let land_mask = smoothstep(0.0, 0.2, base);
    let detail = fbm(dir * DETAIL_FREQ, detail_seed, 5, 2.2, 0.5);

    let mut height = base * HEIGHT_AMPLITUDE;
    if height > 0.0 {
        height += detail * HEIGHT_AMPLITUDE * 0.35 * land_mask;
    } else {
        height = height * 0.4 + detail * HEIGHT_AMPLITUDE * 0.08;
        height = height.min(-0.2);
    }

    height
}

fn color_from_height(height: f32) -> [f32; 3] {
    let h = height / HEIGHT_AMPLITUDE;
    if h < -0.18 {
        return [0.03, 0.08, 0.18];
    }
    if h < -0.05 {
        return [0.05, 0.18, 0.32];
    }
    if h < 0.03 {
        return [0.76, 0.7, 0.52];
    }
    if h < 0.4 {
        return [0.14, 0.52, 0.25];
    }
    if h < 0.7 {
        return [0.38, 0.38, 0.34];
    }
    [0.86, 0.86, 0.92]
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fbm(pos: Vec3, seed: u32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..octaves {
        let p = pos * freq;
        sum += sample_noise_3d(p.x, p.y, p.z, seed) * amp;
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }
    sum / norm
}

fn sample_noise_3d(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let z0 = z.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;
    let tz = z - z0 as f32;

    let c000 = hash3(x0, y0, z0, seed);
    let c100 = hash3(x1, y0, z0, seed);
    let c010 = hash3(x0, y1, z0, seed);
    let c110 = hash3(x1, y1, z0, seed);
    let c001 = hash3(x0, y0, z1, seed);
    let c101 = hash3(x1, y0, z1, seed);
    let c011 = hash3(x0, y1, z1, seed);
    let c111 = hash3(x1, y1, z1, seed);

    let x00 = lerp(c000, c100, tx);
    let x10 = lerp(c010, c110, tx);
    let x01 = lerp(c001, c101, tx);
    let x11 = lerp(c011, c111, tx);
    let y0 = lerp(x00, x10, ty);
    let y1 = lerp(x01, x11, ty);
    lerp(y0, y1, tz)
}

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let mut n = (x as u32).wrapping_mul(73856093)
        ^ (y as u32).wrapping_mul(19349663)
        ^ (z as u32).wrapping_mul(83492791)
        ^ seed;
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    n ^= n >> 16;
    (n as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
