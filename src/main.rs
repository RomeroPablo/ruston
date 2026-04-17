use std::sync::Arc;
use std::time::Instant;

use imgui_wgpu::Renderer;
use imgui_wgpu::RendererConfig;
use imgui_winit_support::HiDpiMode;
use imgui_winit_support::WinitPlatform;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::Event;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoop;
use winit::window::Window;
use winit::window::WindowId;

struct ImguiState {
    imgui: imgui::Context,
    platform: WinitPlatform,
    renderer: Renderer,
    last_frame: Instant,
    show_demo: bool,
}

impl ImguiState {
    fn new(window: &Window, gpu: &GpuState) -> Self {
        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);

        let mut platform = WinitPlatform::new(&mut imgui);
        platform.attach_window(imgui.io_mut(), window, HiDpiMode::Default);

        let renderer_config = RendererConfig {
            texture_format: gpu.config.format,
            ..RendererConfig::new()
        };
        let renderer = Renderer::new(&mut imgui, &gpu.device, &gpu.queue, renderer_config);

        Self {
            imgui,
            platform,
            renderer,
            last_frame: Instant::now(),
            show_demo: true,
        }
    }
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
}

impl GpuState {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to find a GPU adapter");

        let caps = surface.get_capabilities(&adapter);
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| {
                matches!(
                    mode,
                    wgpu::CompositeAlphaMode::PreMultiplied
                        | wgpu::CompositeAlphaMode::PostMultiplied
                )
            })
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
            })
            .await
            .expect("failed to create device");

        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("surface is not compatible with adapter");
        config.present_mode = wgpu::PresentMode::Fifo;
        config.alpha_mode = alpha_mode;
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("triangle shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("render pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
        }
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            self.size = new_size;
            return;
        }

        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self, window: &Window, imgui: &mut ImguiState, event_loop: &ActiveEventLoop) {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                self.resize(self.size);
                return;
            }
            Err(wgpu::SurfaceError::Lost) => {
                self.resize(self.size);
                return;
            }
            Err(wgpu::SurfaceError::Timeout) => return,
            Err(wgpu::SurfaceError::OutOfMemory) => {
                event_loop.exit();
                return;
            }
            Err(wgpu::SurfaceError::Other) => return,
        };

        let now = Instant::now();
        imgui.imgui.io_mut().update_delta_time(now - imgui.last_frame);
        imgui.last_frame = now;
        imgui
            .platform
            .prepare_frame(imgui.imgui.io_mut(), window)
            .expect("failed to prepare imgui frame");

        let ui = imgui.imgui.frame();
        ui.show_demo_window(&mut imgui.show_demo);
        imgui.platform.prepare_render(ui, window);
        let draw_data = imgui.imgui.render();

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.00,
                            g: 0.00,
                            b: 0.00,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.render_pipeline);
            pass.draw(0..3, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("imgui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            imgui
                .renderer
                .render(draw_data, &self.queue, &self.device, &mut pass)
                .expect("failed to render imgui");
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

#[derive(Default)]
struct App {
    gpu: Option<GpuState>,
    window: Option<Arc<Window>>,
    imgui: Option<ImguiState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("ruston")
                        .with_inner_size(PhysicalSize::new(800, 600))
                        .with_transparent(false),
                )
                .unwrap(),
        );
        let gpu = pollster::block_on(GpuState::new(window.clone()));
        let imgui = ImguiState::new(window.as_ref(), &gpu);

        self.imgui = Some(imgui);
        self.gpu = Some(gpu);
        self.window = Some(window);

        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        if window.id() != window_id {
            return;
        }

        if let Some(imgui) = self.imgui.as_mut() {
            let wrapped_event: Event<()> = Event::WindowEvent {
                window_id,
                event: event.clone(),
            };
            imgui
                .platform
                .handle_event(imgui.imgui.io_mut(), window.as_ref(), &wrapped_event);
        }

        match event {
            WindowEvent::CloseRequested => {
                self.imgui = None;
                self.gpu = None;
                self.window = None;
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(new_size);
                }
            }
            WindowEvent::RedrawRequested => {
                let Some(window) = self.window.as_ref() else {
                    return;
                };
                let Some(gpu) = self.gpu.as_mut() else {
                    return;
                };
                let Some(imgui) = self.imgui.as_mut() else {
                    return;
                };

                gpu.render(window.as_ref(), imgui, event_loop);
                window.request_redraw();
            }
            _ => {}
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.imgui = None;
        self.gpu = None;
        self.window = None;
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
