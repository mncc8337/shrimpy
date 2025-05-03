mod vec3;
mod tracer_struct;
mod graphics;

use {
    anyhow::Result,
    graphics::Gfx,
    std::sync::Arc,
    winit::{
        application::ApplicationHandler,
        event::{
            DeviceEvent,
            DeviceId,
            ElementState,
            MouseScrollDelta,
            WindowEvent
        },
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::{Window, WindowId}
    },
    crate::vec3::Vec3,
    crate::tracer_struct::{Material, Sphere},
};

struct Shrimpy {
    width: u32,
    height: u32,
    gfx_callback: fn(&mut Gfx),
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

        (self.gfx_callback)(self.gfx.as_mut().unwrap());
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
                _gfx.uniforms.camera.move_foward(-delta);
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

fn scene_build(gfx: &mut Gfx) {
    let mut material1 = Material::new();
    material1.color = Vec3::new(0.3, 0.2, 0.9);

    let mut sphere1 = Sphere::new();
    sphere1.center = Vec3::new(0.0, 0.0, -3.0);
    sphere1.radius = 1.0;
    sphere1.material_id = gfx.scene_add_material(material1);
    gfx.scene_add_sphere(sphere1);

    gfx.scene_update();
}

fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = Shrimpy {
        width: 800,
        height: 600,
        gfx_callback: scene_build,
        window: None,
        gfx: None,
        button_state: [false; 4],
    };

    event_loop.run_app(&mut app)?;

    Ok(())
}
