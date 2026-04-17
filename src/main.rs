use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::event_loop::EventLoop;
use winit::window::Window;
use winit::window::WindowId;

struct GpuState{
    instance: wgpu::Instance,
    surface : wgpu::Surface<'static>,
    adapter : wgpu::Adapter,
    device  : wgpu::Device,
    queue   : wgpu::Queue,
    config  : wgpu::SurfaceConfiguration,
    size    : PhysicalSize<u32>,
    render_pipeline : wgpu::RenderPipeline,
}

impl GpuState{
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions{
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.expect("failed to find a GPU adapter");
        let caps = surface.get_capabilities(&adapter);
        let alpha_mode = caps.alpha_modes.iter().copied().find(|mode| {
            matches!(mode, wgpu::CompositeAlphaMode::PreMultiplied 
                         | wgpu::CompositeAlphaMode::PostMultiplied)})
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor{
            label: Some("device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::default(),
            experimental_features: wgpu::ExperimentalFeatures::default(),
        }).await.expect("failed to create device");
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .expect("Surface is not compatible with adapter");
        config.present_mode = wgpu::PresentMode::Fifo;
        config.alpha_mode = alpha_mode;
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("triangle shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(
                                                &wgpu::PipelineLayoutDescriptor{
            label: Some("render pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0
        });

        let render_pipeline = device.create_render_pipeline(
                                                &wgpu::RenderPipelineDescriptor{
            label: Some("render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState { 
                topology: wgpu::PrimitiveTopology::default(), 
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState{
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState{
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self{
            instance,
            surface,
            adapter,
            device,
            queue,
            config,
            size,
            render_pipeline,
        }
    }
    fn resize(&mut self, new_size: PhysicalSize<u32>){
        if new_size.width == 0 || new_size.height == 0 {
            self.size = new_size;
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    fn render(&mut self, event_loop: &ActiveEventLoop){
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => frame,
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                self.resize(self.size);
                frame
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.resize(self.size);
                return;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                event_loop.exit();
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout => return,
            wgpu::CurrentSurfaceTexture::Occluded => return,
            wgpu::CurrentSurfaceTexture::Validation => return,
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor{
            label: Some("render encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor{
                label: Some("main pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations{
                        load: wgpu::LoadOp::Clear(wgpu::Color{
                            r: 0.0,
                            b: 0.0,
                            g: 0.0,
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
        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }
}

#[derive(Default)]
struct App{
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
}

impl ApplicationHandler for App{
    fn resumed(&mut self, event_loop: &ActiveEventLoop){
        if self.window.is_some(){ return; }
        let window = Arc::new(event_loop.create_window(Window::default_attributes()
                .with_title("ruston")
                .with_inner_size(PhysicalSize::new(800, 600))
                .with_transparent(true),
            ).unwrap(),
        );
        let gpu = pollster::block_on(GpuState::new(window.clone()));
        self.gpu = Some(gpu);
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent){
        let Some(window) = self.window.as_ref() else { return; };
        if window.id() != window_id { return };
        match event{
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = self.gpu.as_mut(){ gpu.resize(new_size); }
            }
            WindowEvent::RedrawRequested => { // draw here
                let Some(gpu) = self.gpu.as_mut() else { return; };
                gpu.render(event_loop);
                window.request_redraw();
            }
            _ => {}
        }
    }
}

fn main(){
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
