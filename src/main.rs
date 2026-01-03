use glam::{Mat4, Vec3};
use rand::{rngs::StdRng, SeedableRng};
use std::error::Error;
use std::time::Instant;
use wgpu::{SurfaceError, SurfaceTargetUnsafe};

mod input;
mod depth;
mod terrain;
mod water;
use winit::{
    dpi::PhysicalSize,
    event::{DeviceEvent, Event, MouseButton, WindowEvent},
    event_loop::EventLoop,
    window::{CursorGrabMode, Window, WindowBuilder},
};

const MAP_TRANSITION_SPEED: f32 = 2.5;

#[cfg(feature = "ui")]
use egui_wgpu::ScreenDescriptor;

struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    window: Window,
    size: PhysicalSize<u32>,
    clear: Vec3,
    depth: depth::DepthTexture,
    input: input::InputState,
    map_blend: f32,
    map_target: f32,
    map_rotation: f32,
    last_frame: Instant,
    rng: StdRng,
    terrain: terrain::Terrain,
    water: water::Water,
    #[cfg(feature = "ui")]
    gui: Gui,
}

impl State {
    async fn new(event_loop: &EventLoop<()>) -> Result<Self, Box<dyn Error>> {
        let window = WindowBuilder::new()
            .with_title("wgpu + winit bootstrap")
            .build(event_loop)?;

        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = unsafe {
            instance.create_surface_unsafe(SurfaceTargetUnsafe::from_window(&window)?)?
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No suitable GPU adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("wgpu device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let mut rng = StdRng::from_entropy();
        let terrain = terrain::Terrain::new(&device, surface_format, &mut rng);
        let depth = depth::DepthTexture::new(&device, &config);
        let water = water::Water::new(&device, surface_format, terrain::WATER_LEVEL);

        #[cfg(feature = "ui")]
        let gui = Gui::new(&window, &device, surface_format);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            window,
            size,
            clear: Vec3::new(0.05, 0.08, 0.1),
            depth,
            input: input::InputState::new(1.2),
            map_blend: 0.0,
            map_target: 0.0,
            map_rotation: 0.0,
            last_frame: Instant::now(),
            rng,
            terrain,
            water,
            #[cfg(feature = "ui")]
            gui,
        })
    }

    fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth = depth::DepthTexture::new(&self.device, &self.config);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        let mut handled = false;

        match event {
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == MouseButton::Left && *state == winit::event::ElementState::Pressed {
                    self.input.active = true;
                    self.set_cursor_grab(true);
                    handled = true;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                handled |= self.input.handle_scroll(delta);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = glam::Vec2::new(position.x as f32, position.y as f32);
                self.input.handle_cursor_move(pos);
                handled = true;
            }
            WindowEvent::Focused(false) => {
                self.input.active = false;
                self.set_cursor_grab(false);
            }
            WindowEvent::Focused(true) => {
                self.input.active = false;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                handled |= self.input.handle_key(event);
            }
            _ => {}
        }

        #[cfg(feature = "ui")]
        {
            if self.gui.on_event(&self.window, event) {
                handled = true;
            }
        }

        handled
    }

    fn set_cursor_grab(&self, grab: bool) {
        if grab {
            if self
                .window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| self.window.set_cursor_grab(CursorGrabMode::Confined))
                .is_err()
            {
                eprintln!("Could not lock cursor");
            }
            let _ = self.window.set_cursor_visible(false);
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            let _ = self.window.set_cursor_visible(true);
        }
    }

    fn toggle_map(&mut self) {
        if self.map_target < 0.5 {
            let dir = self.input.position.normalize_or_zero();
            let lon_center = dir.z.atan2(dir.x);
            self.map_rotation = std::f32::consts::PI - lon_center;
            self.map_target = 1.0;
        } else {
            self.map_target = 0.0;
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;

        self.input.update(dt);
        if self.input.take_toggle_map() {
            self.toggle_map();
        }

        let blend_step = 1.0 - (-MAP_TRANSITION_SPEED * dt).exp();
        self.map_blend += (self.map_target - self.map_blend) * blend_step;
        self.map_blend = self.map_blend.clamp(0.0, 1.0);
        let aspect = self.config.width.max(1) as f32 / self.config.height.max(1) as f32;
        let eye = self.input.position;
        let orbit = eye.length().max(1.0);
        let up = Vec3::Y;
        let view = Mat4::look_at_rh(eye, Vec3::ZERO, up);
        let near = 1.0;
        let far = (orbit + terrain::WORLD_RADIUS * 4.0).max(terrain::WORLD_RADIUS * 6.0);
        let proj = Mat4::perspective_rh(50f32.to_radians(), aspect, near, far);
        let view_proj = proj * view;
        self.terrain
            .update_view(&self.queue, view_proj, self.map_blend, self.map_rotation);
        self.water
            .update_view(&self.queue, view_proj, self.map_blend, self.map_rotation);

        if self.input.take_randomize() {
            self.terrain.randomize(&self.queue, &mut self.rng);
        }
    }

    fn render(&mut self) -> Result<(), SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("render encoder"),
                });

        let clear = wgpu::Color {
            r: self.clear.x as f64,
            g: self.clear.y as f64,
            b: self.clear.z as f64,
            a: 1.0,
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.terrain.draw(&mut pass);
            self.water.draw(&mut pass);
        }

        #[cfg(feature = "ui")]
        let ui_frame = self.gui.draw(
            &self.window,
            &self.device,
            &self.queue,
            &view,
            &mut encoder,
            &self.config,
            self.map_target,
            self.terrain.settings(),
        );

        #[cfg(feature = "ui")]
        {
            let mut submits = ui_frame.commands;
            submits.push(encoder.finish());
            self.queue.submit(submits);
            if ui_frame.settings_changed {
                self.terrain
                    .set_settings(&self.queue, ui_frame.settings);
            }
            if ui_frame.randomize {
                self.terrain.randomize(&self.queue, &mut self.rng);
            }
            if ui_frame.toggle_map {
                self.toggle_map();
            }
        }
        #[cfg(not(feature = "ui"))]
        self.queue.submit(Some(encoder.finish()));

        frame.present();
        Ok(())
    }
}

#[cfg(feature = "ui")]
struct Gui {
    ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

#[cfg(feature = "ui")]
struct UiFrame {
    commands: Vec<wgpu::CommandBuffer>,
    randomize: bool,
    toggle_map: bool,
    settings: terrain::TerrainSettings,
    settings_changed: bool,
}

#[cfg(feature = "ui")]
impl Gui {
    fn new(window: &Window, device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
        );
        let renderer = egui_wgpu::Renderer::new(device, surface_format, None, 1);
        Self {
            ctx,
            state,
            renderer,
        }
    }

    fn on_event(&mut self, window: &Window, event: &WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }

    fn draw(
        &mut self,
        window: &Window,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        surface_config: &wgpu::SurfaceConfiguration,
        map_target: f32,
        settings: terrain::TerrainSettings,
    ) -> UiFrame {
        let raw_input = self.state.take_egui_input(window);
        let mut randomize = false;
        let mut toggle_map = false;
        let mut settings = settings;
        let mut settings_changed = false;
        let full_output = self.ctx.run(raw_input, |ctx| {
            egui::Window::new("Overlay")
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Procedural terrain");
                    let label = if map_target < 0.5 {
                        "Flatten Map"
                    } else {
                        "Globe View"
                    };
                    if ui.button(label).clicked() {
                        toggle_map = true;
                    }
                    if ui.button("Randomise").clicked() {
                        randomize = true;
                    }
                    ui.separator();
                    ui.label("Sand & biomes");
                    settings_changed |= ui
                        .add(
                            egui::Slider::new(&mut settings.beach_max_height, 0.0..=0.08)
                                .text("Beach height"),
                        )
                        .changed();
                    settings_changed |= ui
                        .add(
                            egui::Slider::new(&mut settings.desert_moisture_max, 0.0..=1.0)
                                .text("Desert moisture max"),
                        )
                        .changed();
                    settings_changed |= ui
                        .add(
                            egui::Slider::new(&mut settings.semi_arid_moisture_max, 0.0..=1.0)
                                .text("Semi-arid moisture max"),
                        )
                        .changed();
                    settings_changed |= ui
                        .add(
                            egui::Slider::new(&mut settings.land_elevation_bias, -0.05..=0.15)
                                .text("Land elevation bias"),
                        )
                        .changed();
                    if settings.semi_arid_moisture_max < settings.desert_moisture_max {
                        settings.semi_arid_moisture_max = settings.desert_moisture_max;
                        settings_changed = true;
                    }
                });
        });

        self.state
            .handle_platform_output(window, full_output.platform_output);

        for (id, delta) in full_output.textures_delta.set {
            self.renderer.update_texture(device, queue, id, &delta);
        }

        let pixels_per_point = egui_winit::pixels_per_point(&self.ctx, window);
        let primitives = self
            .ctx
            .tessellate(full_output.shapes, pixels_per_point);
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [surface_config.width, surface_config.height],
            pixels_per_point,
        };
        let user_cmd_bufs = self.renderer.update_buffers(
            device,
            queue,
            encoder,
            &primitives,
            &screen_descriptor,
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            self.renderer
                .render(&mut pass, &primitives, &screen_descriptor);
        }

        for id in full_output.textures_delta.free {
            self.renderer.free_texture(&id);
        }

        UiFrame {
            commands: user_cmd_bufs,
            randomize,
            toggle_map,
            settings,
            settings_changed,
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    let mut state = pollster::block_on(State::new(&event_loop))?;

    event_loop.run(move |event, elwt| {
        match event {
            Event::DeviceEvent { event, .. } => {
                if let DeviceEvent::MouseMotion { delta } = event {
                    state.input.handle_mouse_delta(delta);
                }
            }
            Event::WindowEvent { event, window_id }
                if window_id == state.window().id() =>
            {
                if !state.input(&event) {
                    match event {
                        WindowEvent::CloseRequested => elwt.exit(),
                        WindowEvent::Resized(size) => state.resize(size),
                        WindowEvent::ScaleFactorChanged { .. } => {
                            state.resize(state.window().inner_size());
                        }
                        WindowEvent::RedrawRequested => {
                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                                    state.resize(state.size)
                                }
                                Err(SurfaceError::OutOfMemory) => elwt.exit(),
                                Err(e) => eprintln!("render error: {e:?}"),
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => state.window().request_redraw(),
            _ => {}
        }
    })?;

    Ok(())
}
