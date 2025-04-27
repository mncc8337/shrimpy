use {
    std::sync::Arc,
    std::time::Instant,
    anyhow::{Context, Result},
    bytemuck::{Pod, Zeroable},
    wgpu,
    winit::window::Window,
};

#[derive(Copy, Clone, Pod, Zeroable)]
#[repr(C)]
struct Uniforms {
    width: u32,
    height: u32,
    elapsed_seconds: f32,
}

pub struct Gfx {
    pub surface: wgpu::Surface<'static>,
    pub start_time: Instant,

    device: wgpu::Device,
    queue: wgpu::Queue,

    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,

    render_pipeline: wgpu::RenderPipeline,
    render_bind_group: wgpu::BindGroup,
}

impl Gfx {
    pub fn new(window: Arc<Window>) -> Self {
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
            width: window_size.width,
            height: window_size.height,
            elapsed_seconds: 0.0,
        };
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (bind_group_layout, render_pipeline) = Gfx::create_pipeline(
            &device,
            &Gfx::compile_shader_module(&device),
            texture_format
        );
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        Self {
            surface,
            start_time,

            device,
            queue,

            uniforms,
            uniform_buffer,

            render_pipeline,
            render_bind_group,
        }
    }

    fn compile_shader_module(device: &wgpu::Device) -> wgpu::ShaderModule {
        use std::borrow::Cow;

        let code = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders.wgsl"));

        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(code)),
        })
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

    pub fn render_frame(&mut self) {
        let elapsed = self.start_time.elapsed().as_millis();
        self.uniforms.elapsed_seconds = elapsed as f32 / 1000.0;

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

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.uniforms)
        );

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.render_bind_group, &[]);

        render_pass.draw(0..6, 0..1);

        drop(render_pass);

        let command_buffer = encoder.finish();
        self.queue.submit(Some(command_buffer));

        frame.present();
    }
}
