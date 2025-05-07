use {
    crate::vec3::Vec3,
    bytemuck::{Pod, Zeroable},
};

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
// size 64
pub struct Camera {
    pub position: Vec3,
    _pad0: u32,
    pub direction: Vec3,
    pub fov: f32,
    pub width: f32,
    pub focus_distance: f32,
    pub apeture: f32,
    pub diverge_strength: f32,
    pub max_ray_bounces: u32,
    _pad1: [u32; 3]
}

impl Camera {
    pub fn new() -> Self {
        Camera {
            position: Vec3::zero(),
            _pad0: 0,
            direction: Vec3::new(0.0, 0.0, -1.0),
            fov: 75.0 * 0.01745329251,
            width: 1.0,
            focus_distance: 2.0,
            apeture: 0.02,
            diverge_strength: 0.004,
            max_ray_bounces: 50,
            _pad1: [0; 3],
        }
    }

    pub fn get_right_direction(&self) -> Vec3 {
        let world_up = Vec3::new(0.0, 1.0, 0.0);

        -self.direction.cross(&world_up).normalized()
    }

    pub fn get_up_direction(&self) -> Vec3 {
        self.direction.cross(&self.get_right_direction()).normalized()
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
    pub fn new(color: Vec3, roughness_or_ior: f32, emission_strength: f32, volume_density: f32) -> Self {
        Self {
            color,
            roughness_or_ior,
            emission_strength,
            volume_density,
            _pad0: [0; 2],
        }
    }

    pub fn default() -> Self {
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
    pub fn new(center: Vec3, radius: f32, material_id: u32) -> Self {
        Self {
            center,
            radius,
            material_id,
            _pad0: [0; 3],
        }
    }

    pub fn default() -> Self {
        Self {
            radius: 1.0,
            material_id: 0,
            center: Vec3::zero(),
            _pad0: [0; 3],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
// size 64
pub struct Triangle {
    pub vertex_0: Vec3,
    _pad0: u32,
    pub vertex_1: Vec3,
    _pad1: u32,
    pub vertex_2: Vec3,
    _pad2: u32,
    pub material_id: u32,
    _pad3: [u32; 3],
}

impl Triangle {
    pub fn new(vertices: [Vec3; 3], material_id: u32) -> Self {
        Self {
            vertex_0: vertices[0],
            _pad0: 0,
            vertex_1: vertices[1],
            _pad1: 0,
            vertex_2: vertices[2],
            _pad2: 0,
            material_id,
            _pad3: [0; 3],
        }
    }

    pub fn default() -> Self {
        Self {
            vertex_0: Vec3::zero(),
            _pad0: 0,
            vertex_1: Vec3::zero(),
            _pad1: 0,
            vertex_2: Vec3::zero(),
            _pad2: 0,
            material_id: 0,
            _pad3: [0; 3],
        }
    }

    pub fn bounding_box(self) -> (Vec3, Vec3) {
        let mut bbox_min = self.vertex_0;
        let mut bbox_max = self.vertex_0;

        bbox_min[0] = bbox_min[0].min(self.vertex_1[0]);
        bbox_min[0] = bbox_min[0].min(self.vertex_2[0]);

        bbox_min[1] = bbox_min[1].min(self.vertex_1[1]);
        bbox_min[1] = bbox_min[1].min(self.vertex_2[1]);

        bbox_min[2] = bbox_min[2].min(self.vertex_1[2]);
        bbox_min[2] = bbox_min[2].min(self.vertex_2[2]);

        bbox_max[0] = bbox_max[0].max(self.vertex_1[0]);
        bbox_max[0] = bbox_max[0].max(self.vertex_2[0]);

        bbox_max[1] = bbox_max[1].max(self.vertex_1[1]);
        bbox_max[1] = bbox_max[1].max(self.vertex_2[1]);

        bbox_max[2] = bbox_max[2].max(self.vertex_1[2]);
        bbox_max[2] = bbox_max[2].max(self.vertex_2[2]);

        (bbox_min, bbox_max)
    }

    pub fn center(self) -> Vec3 {
        (self.vertex_0 + self.vertex_1 + self.vertex_2) / 3.0
    }
}

const TRIANGLES_PER_LEAF: usize = 7;

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
// size 64
pub struct BVHNode {
    pub bbox_min: Vec3,
    pub child1: u32,
    pub bbox_max: Vec3,
    pub child2: u32,
    pub triangle_count: u32,
    pub triangle_ids: [u32; TRIANGLES_PER_LEAF],
    // _pad0: [u32; 3],
}

impl BVHNode {
    pub fn default() -> Self {
        Self {
            bbox_min: Vec3::zero(),
            child1: 0,
            bbox_max: Vec3::zero(),
            child2: 0,
            triangle_count: 0,
            triangle_ids: [0; TRIANGLES_PER_LEAF],
            // _pad0: [0; 3],
        }
    }

    pub fn bvh_build(
        tris: &mut [Triangle],
        tri_indices: &mut [usize],
        tree: &mut Vec<BVHNode>,
        max_triangles_per_leaf: usize
    ) -> u32 {
        let node_index = tree.len() as u32;

        // compute bbox for current node
        let mut bbox_min = Vec3::all(f32::INFINITY);
        let mut bbox_max = Vec3::all(f32::NEG_INFINITY);
        for i in tri_indices.iter() {
            let (tris_bbox_min, tris_bbox_max) = tris[*i].bounding_box();
            bbox_min = bbox_min.min(tris_bbox_min);
            bbox_max = bbox_max.max(tris_bbox_max);
        }

        for i in 0..3  {
            if (bbox_max[i] - bbox_min[i]).abs() < 1e-4 {
                bbox_max[i] += 0.01;
                bbox_min[i] -= 0.01;
            }
        }

        // create leaf node
        if tri_indices.len() <= TRIANGLES_PER_LEAF {
            let mut node = BVHNode::default();
            node.bbox_min = bbox_min;
            node.bbox_max = bbox_max;
            node.triangle_count = tri_indices.len() as u32;
            node.triangle_ids = {
                let mut triangle_ids = [0; TRIANGLES_PER_LEAF];
                for i in 0..tri_indices.len() {
                    triangle_ids[i] = tri_indices[i] as u32;
                }

                triangle_ids
            };
            tree.push(node);

            return node_index;
        }

        // find longest axis
        let dbox = bbox_max - bbox_min;
        let axis = if dbox[0] > dbox[1] && dbox[0] > dbox[2] {
            0
        } else if dbox[1] > dbox[2] {
            1
        } else {
            2
        };

        // sort along axis
        tri_indices.sort_by(|&a, &b| {
            let a_center = &tris[a].center();
            let b_center = &tris[b].center();
            a_center[axis].partial_cmp(&b_center[axis]).unwrap()
        });

        // push dummy parent node before creating children
        // to preserve node_index
        tree.push(BVHNode::default());

        let mid = tri_indices.len() / 2;
        let (left_indices, right_indices) = tri_indices.split_at_mut(mid);

        let child1 = BVHNode::bvh_build(tris, left_indices, tree, max_triangles_per_leaf);
        let child2 = BVHNode::bvh_build(tris, right_indices, tree, max_triangles_per_leaf);

        // update parent node
        let current_node = &mut tree[node_index as usize];
        current_node.child1 = child1;
        current_node.child2 = child2;
        current_node.bbox_min = bbox_min;
        current_node.bbox_max = bbox_max;
        current_node.triangle_count = 0;
        current_node.triangle_ids = [0; TRIANGLES_PER_LEAF];

        node_index
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Scene {
    pub materials: [Material; 64],
    pub spheres: [Sphere; 64],
    pub triangles: [Triangle; 256],
    pub sphere_count: u32,
    pub triangle_count: u32,
    _pad0: [u32; 2],
    pub bvh: [BVHNode; 96],
}

impl Scene {
    pub fn new() -> Self {
        Self {
            materials: [Material::default(); 64],
            spheres: [Sphere::default(); 64],
            triangles: [Triangle::default(); 256],
            sphere_count: 0,
            triangle_count: 0,
            _pad0: [0; 2],
            bvh: [BVHNode::default(); 96],
        }
    }
}
