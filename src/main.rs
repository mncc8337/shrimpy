use {
    anyhow::Result,
    winit::{
        event::WindowEvent,
        event_loop::{ControlFlow, EventLoop, ActiveEventLoop},
        application::ApplicationHandler,
        window::{Window, WindowId},
    },
};

const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;

#[derive(Default)]
struct MantaRay {
    window: Option<Window>,
}

impl ApplicationHandler for MantaRay {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_inner_size(winit::dpi::PhysicalSize::new(WIDTH, HEIGHT))
            .with_resizable(false)
            .with_title("mantaray".to_string());
        self.window = Some(event_loop.create_window(window_attributes).unwrap());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = MantaRay::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}
