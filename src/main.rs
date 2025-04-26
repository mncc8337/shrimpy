use {
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

#[derive(Default)]
struct GPUInfo {
    device: Option<Device>,
    queue: Option<Queue>,
    surface: Option<Surface<'static>>,
}

#[derive(Default)]
struct Shrimpy {
    window: Option<Window>,
    gpu_info: Option<GPUInfo>,
}

impl ApplicationHandler for Shrimpy {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_inner_size(winit::dpi::PhysicalSize::new(WIDTH, HEIGHT))
            .with_resizable(false)
            .with_title("Shrimpy".to_string());
        self.window = Some(event_loop.create_window(window_attributes).unwrap());
        pollster::block_on(async {
            use wgpu::TextureFormat::{Bgra8Unorm, Rgba8Unorm};

            let window = self.window
                .as_ref()
                .context("Window is not initialized").unwrap();

            let instance = wgpu::Instance::default();
            let surface = instance.create_surface(window).unwrap();

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

            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .into_iter()
                .find(|it| matches!(it, Rgba8Unorm | Bgra8Unorm))
                .context("could not find preferred texture format (Rgba8Unorm or Bgra8Unorm)").unwrap();

            let size = window.inner_size();
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

            self.gpu_info = Some(GPUInfo {
                device: Some(device),
                queue: Some(queue),
                surface: Some(unsafe {std::mem::transmute::<Surface<'_>, Surface<'static>>(surface)}),
            })
        })
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
                    .as_ref()
                    .unwrap()
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
