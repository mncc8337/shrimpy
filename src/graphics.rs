use {
    crate::camera::Camera,
    // crate::vec3::Vec3,
    anyhow::Context,
    bytemuck::{Pod, Zeroable},
    core::str,
    std::{borrow::Cow, sync::Arc, time::Instant},
    wgpu,
    winit::window::Window
};

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    camera: Camera,
    // ^ size 64, align 16
    width: u32,
    height: u32,
    elapsed_seconds: f32,
    frame_count: u32,
    pub gamma_correction: f32,
    _pad0: [u32; 3],
    // ^ size 32, align 4
}

pub struct Gfx {
    pub surface: wgpu::Surface<'static>,
    pub start_time: Instant,

    device: wgpu::Device,
    queue: wgpu::Queue,

    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,

    screen_texture: [wgpu::Texture; 2],

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
            // ^
            width: window_size.width,
            height: window_size.height,
            elapsed_seconds: 0.0,
            frame_count: 0,
            gamma_correction: 2.2,
            _pad0: [0; 3],
            // ^
        };
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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

        let screen_texture = Gfx::create_texture(&device, window_size.width, window_size.height);
        let render_bind_group = Gfx::create_bind_groups(
            &device,
            &bind_group_layout,
            &screen_texture,
            &uniform_buffer,
        );

        Self {
            surface,
            start_time,

            device,
            queue,

            uniforms,
            uniform_buffer,

            screen_texture,

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
                    binding: 2,
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
                        resource: wgpu::BindingResource::TextureView(&views[0]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
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
                        resource: wgpu::BindingResource::TextureView(&views[1]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
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
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        };

        [device.create_texture(desc), device.create_texture(desc)]
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
}
