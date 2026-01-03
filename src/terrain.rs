use glam::{Mat4, Vec3};
use rand::Rng;
use std::num::NonZeroU64;
use wgpu::util::DeviceExt;

pub const GRID: u32 = 256;
pub const WORLD_RADIUS: f32 = 100.0;
pub const HEIGHT_AMPLITUDE: f32 = 10.0;
pub const WATER_LEVEL: f32 = 0.0;
const LON_POINTS: u32 = GRID + 1;
const LAT_POINTS: u32 = GRID;
const MAP_WIDTH: f32 = WORLD_RADIUS * std::f32::consts::TAU;
const MAP_HEIGHT: f32 = WORLD_RADIUS * std::f32::consts::PI;
const CONTINENT_FREQ: f32 = 1.0;
const HILL_FREQ: f32 = 8.2;
const MOUNTAIN_FREQ: f32 = 10.2;
const DETAIL_FREQ: f32 = 19.0;
const WARP_FREQ: f32 = 0.75;
const MOISTURE_FREQ: f32 = 0.8;
const SEA_THRESHOLD: f32 = 0.3;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    flat_pos: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x3,
        3 => Float32x3
    ];

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
    morph: [f32; 4],
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
                morph: [0.0; 4],
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
    let hill_seed = rng.gen::<u32>();
    let mountain_seed = rng.gen::<u32>();
    let detail_seed = rng.gen::<u32>();
    let moisture_seed = rng.gen::<u32>();
    let warp_seed = rng.gen::<u32>();

    let mut heights = vec![0.0f32; (LAT_POINTS * LON_POINTS) as usize];
    let mut moisture_map = vec![0.0f32; (LAT_POINTS * LON_POINTS) as usize];
    let mut positions = vec![Vec3::ZERO; (LAT_POINTS * LON_POINTS) as usize];
    let mut flat_positions = vec![Vec3::ZERO; (LAT_POINTS * LON_POINTS) as usize];
    for z in 0..LAT_POINTS {
        let v = z as f32 / (LAT_POINTS - 1) as f32;
        let lat = v * std::f32::consts::PI;
        let sin_lat = lat.sin();
        let cos_lat = lat.cos();
        for x in 0..LON_POINTS {
            let u = x as f32 / (LON_POINTS - 1) as f32;
            let lon = u * std::f32::consts::TAU;
            let dir = Vec3::new(lon.cos() * sin_lat, cos_lat, lon.sin() * sin_lat);
            let (height, moisture) = height_for_dir(
                dir,
                continent_seed,
                hill_seed,
                mountain_seed,
                detail_seed,
                moisture_seed,
                warp_seed,
            );
            let idx = (z * LON_POINTS + x) as usize;
            heights[idx] = height;
            moisture_map[idx] = moisture;
            positions[idx] = dir * (WORLD_RADIUS + height);
            flat_positions[idx] = Vec3::new(u, v, height);
        }
    }

    let mut normals = vec![Vec3::ZERO; positions.len()];
    let lon_last = LON_POINTS - 1;
    for z in 0..LAT_POINTS {
        for x in 0..LON_POINTS {
            let idx = (z * LON_POINTS + x) as usize;
            if z == 0 || z == LAT_POINTS - 1 {
                normals[idx] = positions[idx].normalize_or_zero();
                continue;
            }

            let x_left = if x == 0 { lon_last - 1 } else { x - 1 };
            let x_right = if x == lon_last { 1 } else { x + 1 };
            let left = positions[(z * LON_POINTS + x_left) as usize];
            let right = positions[(z * LON_POINTS + x_right) as usize];
            let down = positions[((z - 1) * LON_POINTS + x) as usize];
            let up = positions[((z + 1) * LON_POINTS + x) as usize];
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

    let mut vertices = Vec::with_capacity((LAT_POINTS * LON_POINTS) as usize);
    for idx in 0..positions.len() {
        let height = heights[idx];
        let color = color_from_height(height, positions[idx].normalize_or_zero(), moisture_map[idx]);
        vertices.push(Vertex {
            pos: positions[idx].into(),
            normal: normals[idx].into(),
            color,
            flat_pos: flat_positions[idx].into(),
        });
    }

    let mut indices = Vec::with_capacity(((LAT_POINTS - 1) * (LON_POINTS - 1) * 6) as usize);
    for z in 0..LAT_POINTS - 1 {
        for x in 0..LON_POINTS - 1 {
            let i0 = z * LON_POINTS + x;
            let i1 = i0 + 1;
            let i2 = i0 + LON_POINTS;
            let i3 = i2 + 1;
            indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
        }
    }

    (vertices, indices)
}

fn height_for_dir(
    dir: Vec3,
    continent_seed: u32,
    hill_seed: u32,
    mountain_seed: u32,
    detail_seed: u32,
    moisture_seed: u32,
    warp_seed: u32,
) -> (f32, f32) {
    let warped = (dir + warp_dir(dir, warp_seed)).normalize_or_zero();
    let continent = fbm(warped * CONTINENT_FREQ, continent_seed, 5, 2.05, 0.5) * 0.85
        + fbm(warped * (CONTINENT_FREQ * 0.5), continent_seed ^ 0x9e37, 3, 2.2, 0.5) * 0.15;
    let base = continent - SEA_THRESHOLD;
    let land_mask = smoothstep(0.0, 0.2, base);
    let hills = remap01(fbm(warped * HILL_FREQ, hill_seed, 4, 2.1, 0.5));
    let ridges = ridged_fbm(warped * MOUNTAIN_FREQ, mountain_seed, 4, 2.0, 0.5);
    let detail = fbm(warped * DETAIL_FREQ, detail_seed, 3, 2.4, 0.55);
    let moisture = remap01(
        fbm(warped * MOISTURE_FREQ, moisture_seed, 4, 2.0, 0.5) * 0.8
            + fbm(warped * 0.25, moisture_seed ^ 0x85eb, 2, 2.0, 0.5) * 0.2,
    );

    let mut height = base * HEIGHT_AMPLITUDE;
    if height > 0.0 {
        let inland = smoothstep(0.08, 0.45, base);
        height += (hills - 0.5) * HEIGHT_AMPLITUDE * 0.35 * land_mask;
        height += ridges.powf(2.0) * HEIGHT_AMPLITUDE * 0.9 * inland;
        height += detail * HEIGHT_AMPLITUDE * 0.07;
    } else {
        let depth = (-base).max(0.0);
        let shelf = smoothstep(0.03, 0.2, depth);
        let ocean = -depth.powf(1.35) * HEIGHT_AMPLITUDE * 0.85;
        let shelf_lift = (1.0 - shelf) * HEIGHT_AMPLITUDE * 0.08;
        height = ocean + shelf_lift + detail * HEIGHT_AMPLITUDE * 0.04;
        height = height.min(-0.15);
    }

    (height, moisture)
}

fn color_from_height(height: f32, dir: Vec3, moisture: f32) -> [f32; 3] {
    let h = height / HEIGHT_AMPLITUDE;
    if h < -0.45 {
        return [0.02, 0.05, 0.12];
    }
    if h < -0.2 {
        return [0.03, 0.1, 0.22];
    }
    if h < -0.02 {
        return [0.06, 0.22, 0.35];
    }
    if h < 0.04 {
        return [0.78, 0.72, 0.54];
    }

    let latitude = dir.y.abs();
    let temp = (1.0 - latitude - h * 0.45).clamp(0.0, 1.0);
    if h > 0.85 || temp < 0.15 {
        return [0.9, 0.92, 0.96];
    }
    if h > 0.65 {
        return [0.48, 0.46, 0.44];
    }
    if temp < 0.3 {
        return [0.62, 0.66, 0.6];
    }

    if moisture < 0.22 {
        [0.8, 0.72, 0.45]
    } else if moisture < 0.5 {
        [0.22, 0.56, 0.28]
    } else {
        [0.08, 0.43, 0.22]
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn remap01(v: f32) -> f32 {
    (v * 0.5 + 0.5).clamp(0.0, 1.0)
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

fn ridged_fbm(pos: Vec3, seed: u32, octaves: u32, lacunarity: f32, gain: f32) -> f32 {
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..octaves {
        let p = pos * freq;
        let n = 1.0 - sample_noise_3d(p.x, p.y, p.z, seed).abs();
        let ridge = n * n;
        sum += ridge * amp;
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }
    (sum / norm).clamp(0.0, 1.0)
}

fn warp_dir(dir: Vec3, seed: u32) -> Vec3 {
    let wx = fbm(dir * WARP_FREQ, seed, 3, 2.0, 0.5);
    let wy = fbm(dir * WARP_FREQ + Vec3::splat(12.7), seed ^ 0x27d4, 3, 2.0, 0.5);
    let wz = fbm(dir * WARP_FREQ + Vec3::splat(31.4), seed ^ 0x1656, 3, 2.0, 0.5);
    Vec3::new(wx, wy, wz) * 0.35
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
