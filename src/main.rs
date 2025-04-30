mod vec3;
mod camera;
mod graphics;

use {
    anyhow::Result, graphics::Gfx, std::sync::Arc, winit::{
        application::ApplicationHandler,
        event::{
            DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent
        },
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId}
    }
};

#[derive(Default)]
struct Shrimpy {
    width: u32,
    height: u32,
    window: Option<Arc<Window>>,
    gfx: Option<Gfx>,
    button_state: [bool; 4],
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
                self.gfx.as_mut().unwrap().render_frame();

                self.window.as_ref().unwrap().request_redraw();
            },
            _ => (),
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: DeviceId, event: DeviceEvent) {
        match event {
            DeviceEvent::MouseWheel { delta } => {
                let delta = match delta {
                    MouseScrollDelta::PixelDelta(delta) => 0.001 * delta.y as f32,
                    MouseScrollDelta::LineDelta(_, y) => y * 0.001,
                };
                let _gfx = self.gfx.as_mut().unwrap();
                _gfx.uniforms.camera.move_foward(delta);
                _gfx.render_reset()
            },
            DeviceEvent::Button { button, state } => {
                self.button_state[button as usize] = state == ElementState::Pressed;
                if state == ElementState::Pressed && button == 2 {
                    pollster::block_on(async {
                        self.gfx.as_mut().unwrap().save_render().await;
                    });
                }
            },
            DeviceEvent::MouseMotion { delta: (dx, dy) } => {
                let _gfx = self.gfx.as_mut().unwrap();
                if self.button_state[3] {
                    _gfx.uniforms.camera.pan(-dx as f32 * 0.004);
                    _gfx.uniforms.camera.tilt(dy as f32 * 0.004);
                    _gfx.render_reset()
                } else if self.button_state[1] {
                    _gfx.uniforms.camera.move_up(dy as f32 * 0.004);
                    _gfx.uniforms.camera.move_right(-dx as f32 * 0.004);
                    _gfx.render_reset()
                }
            },
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
