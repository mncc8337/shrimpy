use {
    bytemuck::{Pod, Zeroable},
    crate::vec3::Vec3,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
// size 64
pub struct Camera {
    pub position: Vec3,
    _pad0: u32,
    pub direction: Vec3,
    pub width: f32,
    pub fov: f32,
    pub focus_distance: f32,
    pub apeture: f32,
    pub diverge_strength: f32,
    pub max_ray_bounces: u32,
    _pad1: [u32; 3]
}

impl Camera {
    pub fn new() -> Self {
        Camera {
            position: Vec3::all(0.0),
            _pad0: 0,
            direction: Vec3::new(0.0, 0.0, -1.0),
            width: 2.5,
            fov: 75.0 * 0.01745329251,
            focus_distance: 2.0,
            apeture: 0.12,
            diverge_strength: 0.004,
            max_ray_bounces: 50,
            _pad1: [0; 3],
        }
    }

    pub fn get_right_direction(&self) -> Vec3 {
        let world_up = Vec3::new(0.0, 1.0, 0.0);

        -self.direction.cross(&world_up)
    }

    pub fn get_up_direction(&self) -> Vec3 {
        self.direction.cross(&self.get_right_direction())
    }

    pub fn move_foward(&mut self, ammount: f32) {
        self.position += self.direction * ammount;
    }

    pub fn move_right(&mut self, ammount: f32) {
        self.position += self.get_right_direction() * ammount;
    }
    
    pub fn move_up(&mut self, ammount: f32) {
        self.position += self.get_up_direction() * ammount;
    }

    // TODO: change this to use an angle instead
    pub fn pan(&mut self, ammount: f32) {
        self.direction += self.get_right_direction() * ammount;
        self.direction = self.direction.normalized();
    }

    // TODO: change this to use an angle instead
    pub fn tilt(&mut self, ammount: f32) {
        self.direction += self.get_up_direction() * ammount;
        self.direction = self.direction.normalized();
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
// size 32
pub struct Material {
    pub color: Vec3,
    pub roughness_or_ior: f32,
    pub emission_strength: f32,
    pub volume_density: f32,
    _pad0: [u32; 2],
}

impl Material {
    pub fn new() -> Self {
        Self {
            color: Vec3::all(1.0),
            roughness_or_ior: 1.0,
            emission_strength: 0.0,
            volume_density: 1.0,
            _pad0: [0; 2],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
// size 32
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
    pub material_id: u32,
    _pad0: [u32; 3],
}

impl Sphere {
    pub fn new() -> Self {
        Self {
            radius: 0.0,
            material_id: 0,
            center: Vec3::all(0.0),
            _pad0: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Scene {
    pub materials: [Material; 64],
    pub spheres: [Sphere; 64],
    pub sphere_count: u32,
    _pad0: [u32; 7],
}

impl Scene {
    pub fn new() -> Self {
        Self {
            materials: [Material::new(); 64],
            spheres: [Sphere::new(); 64],
            sphere_count: 0,
            _pad0: [0; 7],
        }
    }
}
