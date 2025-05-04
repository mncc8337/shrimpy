mod vec3;
mod tracer_struct;
mod graphics;
mod file_load;

use {
    crate::{
        tracer_struct::{Material, Sphere, Triangle},
        vec3::Vec3
    }, anyhow::Result, file_load::load_mesh_from, graphics::Gfx, std::sync::Arc, winit::{
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
    }
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
                let gfx = self.gfx.as_mut().unwrap();
                let camera = gfx.get_camera();
                camera.move_foward(-delta);
                gfx.render_reset()
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
                let gfx = self.gfx.as_mut().unwrap();
                let camera = gfx.get_camera();
                if self.button_state[3] {
                    camera.pan(-dx as f32 * 0.004);
                    camera.tilt(dy as f32 * 0.004);
                    gfx.render_reset()
                } else if self.button_state[1] {
                    camera.move_up(dy as f32 * 0.004);
                    camera.move_right(-dx as f32 * 0.004);
                    gfx.render_reset()
                }
            },
            _ => (),
        }
    }
}

fn scene_build(gfx: &mut Gfx) {
    // materials
    let mut ground_mat = Material::default();
    ground_mat.color = Vec3::new(217.0, 177.0, 104.0) / 255.0;
    ground_mat.roughness_or_ior = 1.0;
    let ground_mat_id = gfx.scene_add_material(ground_mat);

    let mut transparent_mat = Material::default();
    transparent_mat.roughness_or_ior = -1.77;
    let trans_mat_id = gfx.scene_add_material(transparent_mat);

    // scene
    let mut ground = load_mesh_from(
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/plane.obj"),
        ground_mat_id,
    );
    for tri in ground.iter_mut() {
        tri.vertex_0 *= 5.0;
        tri.vertex_1 *= 5.0;
        tri.vertex_2 *= 5.0;
    }
    gfx.scene_add_triangles(&ground);

    let mut sphere1 = Sphere::default();
    sphere1.center = Vec3::new(2.5, 1.0, 0.0);
    sphere1.material_id = trans_mat_id;
    sphere1.radius = 0.7;
    gfx.scene_add_sphere(sphere1);

    let mut sphere2 = Sphere::default();
    sphere2.center = Vec3::new(1.5, 1.0, -2.0);
    sphere2.material_id = ground_mat_id;
    gfx.scene_add_sphere(sphere2);

    let mut dodec = load_mesh_from(
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/dodecahedron.obj"),
        trans_mat_id,
    );
    for tri in dodec.iter_mut() {
        tri.vertex_0 += Vec3::new(0.0, 1.35, 0.0);
        tri.vertex_1 += Vec3::new(0.0, 1.35, 0.0);
        tri.vertex_2 += Vec3::new(0.0, 1.35, 0.0);
    }
    gfx.scene_add_triangles(&dodec);

    gfx.scene_update();

    // camera
    let camera = gfx.get_camera();
    camera.max_ray_bounces = 1000;
    camera.apeture = 0.0;
    camera.position = Vec3::new(0.0, 1.5, 2.0);
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
