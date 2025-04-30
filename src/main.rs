mod vec3;
mod camera;
mod graphics;

use {
    std::sync::Arc,
    anyhow::Result,
    winit::{
        application::ApplicationHandler,
        event::WindowEvent,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId}
    },

    graphics::Gfx,
};

#[derive(Default)]
struct Shrimpy {
    width: u32,
    height: u32,
    window: Option<Arc<Window>>,
    gfx: Option<Gfx>,
}

impl ApplicationHandler for Shrimpy {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_inner_size(winit::dpi::PhysicalSize::new(self.width, self.height))
            .with_resizable(false)
            .with_title("Shrimpy".to_string());

        // let shader_code = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders.wgsl"));
        // for faster testing
        let shader_code = &std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders.wgsl")
        ).unwrap();

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        let gfx = Gfx::new(Arc::clone(&window), shader_code);
        window.request_redraw();

        self.window = Some(window);
        self.gfx = Some(gfx);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                self.gfx
                    .as_mut()
                    .unwrap()
                    .render_frame();

                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = Shrimpy {
        width: 800,
        height: 600,
        ..Default::default()
    };
    event_loop.run_app(&mut app)?;

    Ok(())
}
