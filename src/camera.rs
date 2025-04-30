use {
    bytemuck::{Pod, Zeroable},
    crate::vec3::Vec3,
};

#[derive(Debug, Copy, Clone, Pod, Zeroable)]
#[repr(C)]
// size 64, align 16
pub struct Camera {
    pub width: f32,
    pub fov: f32,
    pub focus_distance: f32,
    pub apeture: f32,
    pub diverge_strength: f32,
    pub max_ray_bounces: u32,
    _pad0: [u32; 2],
    // ^ size 32, align 4
    pub position: Vec3,
    _pad1: u32,
    pub direction: Vec3,
    _pad2: u32,
    // ^ size 32, align 16
}

impl Camera {
    pub fn new() -> Self {
        Camera {
            width: 2.5,
            fov: 75.0 * 0.01745329251,
            focus_distance: 2.0,
            apeture: 0.02,
            diverge_strength: 0.004,
            max_ray_bounces: 100,
            _pad0: [0; 2],
            // ^
            position: Vec3::all(0.0),
            _pad1: 0,
            direction: Vec3::new(0.0, 0.0, 1.0),
            _pad2: 0,
            // ^
        }
    }

    pub fn foward(&mut self, ammount: f32) {
        self.position += self.direction * ammount;
    }
}
