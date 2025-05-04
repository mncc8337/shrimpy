use {
    crate::tracer_struct::Triangle,
    crate::vec3::Vec3,
    std::fs::File,
    std::io::{BufRead, BufReader},
    std::str::FromStr,
};

pub fn load_mesh_from(filename: &str, material_id: u32) -> Vec<Triangle> {
    let mut tris = vec![];

    let file = match File::open(filename) {
        Ok(f) => f,
        Err(_) => {
            println!("failed to load file {}", filename);
            return tris;
        }
    };

    let reader = BufReader::new(file);
    let mut has_texture = false;
    let mut verts: Vec<Vec3> = Vec::new();
    let mut texs: Vec<Vec3> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let trimmed = line.trim();

        if trimmed.starts_with("vt") {
            has_texture = true;
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                let mut v = Vec3::zero();
                v.x = 1.0 - f32::from_str(parts[1]).unwrap_or(0.0);
                v.y = 1.0 - f32::from_str(parts[2]).unwrap_or(0.0);
                texs.push(v);
            }
        } else if trimmed.starts_with('v') {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 4 {
                let mut v = Vec3::zero();
                v.x = f32::from_str(parts[1]).unwrap_or(0.0);
                v.y = f32::from_str(parts[2]).unwrap_or(0.0);
                v.z = f32::from_str(parts[3]).unwrap_or(0.0);
                verts.push(v);
            }
        } else if trimmed.starts_with('f') {
            if has_texture {
                let mut tokens = vec![];
                for token in trimmed.split_whitespace().skip(1) {
                    for t in token.split('/') {
                        tokens.push(t.to_string());
                    }
                }

                if tokens.len() >= 6 {
                    let mut tri = Triangle::default();
                    tri.vertex_0 = verts[tokens[0].parse::<usize>().unwrap() - 1];
                    tri.vertex_1 = verts[tokens[2].parse::<usize>().unwrap() - 1];
                    tri.vertex_2 = verts[tokens[4].parse::<usize>().unwrap() - 1];
                    // tri.vert_texture[0] = texs[tokens[1].parse::<usize>().unwrap() - 1];
                    // tri.vert_texture[1] = texs[tokens[3].parse::<usize>().unwrap() - 1];
                    // tri.vert_texture[2] = texs[tokens[5].parse::<usize>().unwrap() - 1];
                    tri.material_id = material_id;
                    tris.push(tri);
                }
            } else {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 4 {
                    let mut tri = Triangle::default();
                    tri.vertex_0 = verts[parts[1].parse::<usize>().unwrap() - 1];
                    tri.vertex_1 = verts[parts[2].parse::<usize>().unwrap() - 1];
                    tri.vertex_2 = verts[parts[3].parse::<usize>().unwrap() - 1];
                    tri.material_id = material_id;
                    tris.push(tri);
                }
            }
        }
    }

    tris
}
