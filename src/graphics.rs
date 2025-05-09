use {
    crate::tracer_struct::{
        Camera,
        Material,
        Scene,
        Sphere,
        Triangle,
        BVHNode,
    },
    anyhow::Context,
    bytemuck::{Pod, Zeroable},
    chrono::Local,
    std::{borrow::Cow, sync::Arc, time::Instant},
    wgpu,
    winit::window::Window
};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
// size 96
pub struct Uniforms {
    camera: Camera,
    width: u32,
    height: u32,
    elapsed_seconds: f32,
    frame_count: u32,
    pub gamma_correction: f32,
    pub psuedo_chromatic_aberration: f32,
    _pad0: [u32; 2],
}

pub struct Gfx {
    pub surface: wgpu::Surface<'static>,
    pub start_time: Instant,

    device: wgpu::Device,
    queue: wgpu::Queue,

    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,

    pub scene: Scene,
    material_count: u32,
    scene_buffer: wgpu::Buffer,

    radiance_samples: [wgpu::Texture; 2],

    render_pipeline: wgpu::RenderPipeline,
    render_bind_group: [wgpu::BindGroup; 2],
}

impl Gfx {
    pub fn new(window: Arc<Window>, shader_code: &str) -> Self {
        use wgpu::TextureFormat::{Bgra8Unorm, Rgba8Unorm};

        let start_time = Instant::now();

        let window_size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window).unwrap();

        let (device, queue, adapter) = pollster::block_on(async {
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

            (device, queue, adapter)
        });

        let caps = surface.get_capabilities(&adapter);
        let texture_format = caps
            .formats
            .into_iter()
            .find(|it| matches!(it, Rgba8Unorm | Bgra8Unorm))
            .context("could not find preferred texture format (Rgba8Unorm or Bgra8Unorm)").unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: texture_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 3,
        };
        surface.configure(&device, &config);

        let uniforms = Uniforms {
            camera: Camera::new(),
            width: window_size.width,
            height: window_size.height,
            elapsed_seconds: 0.0,
            frame_count: 0,
            gamma_correction: 2.2,
            psuedo_chromatic_aberration: 0.0,
            _pad0: [0; 2],
        };
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let scene = Scene::new();
        let material_count = 0;
        let scene_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene"),
            size: std::mem::size_of::<Scene>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader_code)),
        });

        let (bind_group_layout, render_pipeline) = Gfx::create_pipeline(
            &device,
            &shader_module,
            texture_format
        );

        let radiance_samples = Gfx::create_texture(&device, window_size.width, window_size.height);
        let render_bind_group = Gfx::create_bind_groups(
            &device,
            &bind_group_layout,
            &radiance_samples,
            &uniform_buffer,
            &scene_buffer,
        );

        Self {
            surface,
            start_time,

            device,
            queue,

            uniforms,
            uniform_buffer,

            scene,
            material_count,
            scene_buffer,

            radiance_samples,

            render_pipeline,
            render_bind_group,
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        shader_module: &wgpu::ShaderModule,
        texture_format: wgpu::TextureFormat,
    ) -> (wgpu::BindGroupLayout, wgpu::RenderPipeline) {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage {
                            read_only: true,
                        },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float {
                            filterable: false,
                        },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render"),
            layout: Some(&device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&bind_group_layout],
                ..Default::default()
            })),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                polygon_mode: wgpu::PolygonMode::Fill,
                ..Default::default()
            },
            vertex: wgpu::VertexState {
                module: shader_module,
                entry_point: Some("vs_display"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader_module,
                entry_point: Some("fs_display"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        (bind_group_layout, pipeline)
    }

    fn create_bind_groups(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        textures: &[wgpu::Texture; 2],
        uniform_buffer: &wgpu::Buffer,
        scene_buffer: &wgpu::Buffer,
    ) -> [wgpu::BindGroup; 2] {
        let views = [
            textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        [
            // bind group with view[0] assigned to binding 1 and view[1] assigned to binding 2
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: uniform_buffer,
                            offset: 0,
                            size: None,
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: scene_buffer,
                            offset: 0,
                            size: None,
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&views[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&views[1]),
                    },
                ],
            }),

            // bind group with view[1] assigned to binding 1 and view[0] assigned to binding 2
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: uniform_buffer,
                            offset: 0,
                            size: None,
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: scene_buffer,
                            offset: 0,
                            size: None,
                        }),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&views[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&views[0]),
                    },
                ],
            }),
        ]
    }

    fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> [wgpu::Texture; 2] {
        let desc = &wgpu::TextureDescriptor {
            label: Some("texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING 
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };

        [device.create_texture(desc), device.create_texture(desc)]
    }

    pub fn scene_add_material(&mut self, material: Material) -> u32 {
        self.scene.materials[self.material_count as usize] = material;
        self.material_count += 1;
        
        self.material_count - 1
    }

    pub fn scene_add_sphere(&mut self, sphere: Sphere) {
        self.scene.spheres[self.scene.sphere_count as usize] = sphere;
        self.scene.sphere_count += 1;
    }

    pub fn scene_add_triangles(&mut self, triangles: &[Triangle]) {
        for tri in triangles.iter() {
            self.scene.triangles[self.scene.triangle_count as usize] = *tri;
            self.scene.triangle_count += 1;
        }
    }

    pub fn scene_update(&mut self) {
        self.scene_build();

        self.queue.write_buffer(
            &self.scene_buffer,
            0,
            bytemuck::bytes_of(&self.scene)
        );
    }

    pub fn get_camera(&mut self) -> &mut Camera {
        &mut self.uniforms.camera
    }

    pub fn get_uniforms(&mut self) -> &mut Uniforms {
        &mut self.uniforms
    }

    pub fn render_reset(&mut self) {
        self.uniforms.frame_count = 0;
    }

    pub fn render_frame(&mut self) {
        let elapsed = self.start_time.elapsed().as_millis();
        self.uniforms.elapsed_seconds = elapsed as f32 / 1000.0;
        self.uniforms.frame_count += 1;

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.uniforms)
        );

        let frame = self.surface
            .get_current_texture()
            .expect("failed to get current texture");

        let render_target = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render frame"),
            });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &render_target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(
            0,
            &self.render_bind_group[(self.uniforms.frame_count % 2) as usize],
            &[],
        );

        render_pass.draw(0..6, 0..1);

        drop(render_pass);

        let command_buffer = encoder.finish();
        self.queue.submit(Some(command_buffer));

        frame.present();
    }

    pub async fn save_render(&self) {
        // create buffer for readback
        let buffer_size = (self.uniforms.width * self.uniforms.height * 16) as wgpu::BufferAddress;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Readback Buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Copy Encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.radiance_samples[(self.uniforms.frame_count % 2) as usize],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(16 * self.uniforms.width),
                    rows_per_image: Some(self.uniforms.height),
                },
            },
            wgpu::Extent3d {
                width: self.uniforms.width,
                height: self.uniforms.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        // Map the buffer
        let buffer_slice = buffer.slice(..);
        buffer_slice.map_async(wgpu::MapMode::Read, |_| {});

        let _ = self.device.poll(wgpu::PollType::Wait); // wait for GPU work

        let data = buffer_slice.get_mapped_range();
        let data_f32: &[f32] = bytemuck::cast_slice(&data);
        let mut data_u8 = vec![0 as u8; data_f32.len()];

        // copy and convert data to u8 format
        // TODO: implement other tonemapping technique
        // here im using rgb clampping
        for i in 0..data_f32.len() {
            let converted = data_f32[i] / (self.uniforms.frame_count as f32);
            data_u8[i] = (converted.powf(1.0/self.uniforms.gamma_correction) * 255.0) as u8;
        }

        drop(data);
        buffer.unmap();

        let img: image::ImageBuffer<image::Rgba<u8>, _> = image::ImageBuffer::from_raw(
            self.uniforms.width,
            self.uniforms.height,
            data_u8
        ).ok_or("failed to create ImageBuffer from raw data").unwrap();

        // save as PNG
        let date = Local::now();
        let file = std::fs::File::create(format!("./imgs/{}.png",date.format("%Y-%m-%d-%H-%M-%S"))).unwrap();
        let mut writer = std::io::BufWriter::new(file);
        img.write_to(&mut writer, image::ImageFormat::Png).unwrap();

        println!("image saved");
    }

    fn scene_build(&mut self) {
        let mut tri_indices: Vec<usize> = (0..self.scene.triangle_count as usize).collect();
        let mut tmp_bvh = Vec::new();
        BVHNode::bvh_build(&mut self.scene.triangles, &mut tri_indices, &mut tmp_bvh, 8);

        for (i, node) in tmp_bvh.iter().take(96).enumerate() {
            self.scene.bvh[i] = node.clone();
        }
    }
}
