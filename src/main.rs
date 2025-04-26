use {
    std::sync::Arc,
    anyhow::{Context, Result},
    wgpu::{Device, Queue, Surface, SurfaceError},
    winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId}
    },
};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

struct GPUInfo {
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
}

impl GPUInfo {
    fn new(window: Arc<Window>) -> Self {
        use wgpu::TextureFormat::{Bgra8Unorm, Rgba8Unorm};

        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).unwrap();

        let (device, queue, adapter) = pollster::block_on(async {
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                    compatible_surface: Some(&surface),
                })
                .await
                .context("failed to find a compatible adapter").unwrap();

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default())
                .await
                .context("failed to connect to the GPU").unwrap();

            (device, queue, adapter)
        });

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .into_iter()
            .find(|it| matches!(it, Rgba8Unorm | Bgra8Unorm))
            .context("could not find preferred texture format (Rgba8Unorm or Bgra8Unorm)").unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 3,
        };
        surface.configure(&device, &config);

        Self {device, queue, surface}
    }
}

#[derive(Default)]
struct Shrimpy {
    window: Option<Arc<Window>>,
    gpu_info: Option<GPUInfo>,
}

impl ApplicationHandler for Shrimpy {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_inner_size(winit::dpi::PhysicalSize::new(WIDTH, HEIGHT))
            .with_resizable(false)
            .with_title("Shrimpy".to_string());
        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let gpu_info = GPUInfo::new(Arc::clone(&window));
        window.request_redraw();

        self.window = Some(window);
        self.gpu_info = Some(gpu_info);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                let frame: wgpu::SurfaceTexture = self.gpu_info
                    .as_ref()
                    .unwrap()
                    .surface
                    .get_current_texture()
                    .expect("failed to get current texture");

                // TODO: draw frame

                frame.present();
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = Shrimpy::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}
