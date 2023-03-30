use alloc::borrow::Cow;

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;

#[derive(Default, Clone, Copy)]
pub enum Scaling {
    #[default]
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
}

impl Scaling {
    pub fn next(self) -> Self {
        match self {
            Scaling::Nearest => Scaling::Scale2x,
            Scaling::Scale2x => Scaling::Scale3x,
            Scaling::Scale3x => Scaling::Nearest,
        }
    }
}

pub struct Renderer {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: winit::window::Window,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,

    render_pipeline: wgpu::RenderPipeline,

    // Shader config binds
    dim_uniform: wgpu::Buffer,
    scale_type_uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    scaling: Scaling,

    // Texture binds
    texture: Texture,
    diffuse_bind_group: wgpu::BindGroup,
}

impl Renderer {
    #[allow(clippy::too_many_lines)]
    pub async fn new(window: winit::window::Window, scaling: Scaling) -> anyhow::Result<Self> {
        use {anyhow::Context, wgpu::util::DeviceExt};

        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) }?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("unable to obtain wgpu adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::downlevel_webgl2_defaults(),
                },
                None,
            )
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let texture = Texture::new(&device, PX_WIDTH, PX_HEIGHT, None);

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                        count: None,
                    },
                ],
                label: None,
            });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: None,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
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
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: None,
            });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let scale_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[scaling as u32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scale_uniform_buffer.as_entire_binding(),
                },
            ],
            label: None,
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("../shader/shader.wgsl"))),
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            window,
            config,
            size,
            render_pipeline,
            dim_uniform: uniform_buffer,
            uniform_bind_group,
            scaling,
            scale_type_uniform: scale_uniform_buffer,
            texture,
            diffuse_bind_group,
        })
    }

    pub const fn window(&self) -> &winit::window::Window {
        &self.window
    }

    pub fn cycle_scale_mode(&mut self) {
        self.scaling = self.scaling.next();

        self.queue.write_buffer(
            &self.scale_type_uniform,
            0,
            bytemuck::cast_slice(&[self.scaling as u32]),
        );
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            let width = new_size.width;
            let height = new_size.height;

            let (x, y) = {
                let mul = (width / PX_WIDTH).min(height / PX_HEIGHT);
                #[allow(clippy::cast_precision_loss)]
                let x = (PX_WIDTH * mul) as f32 / width as f32;
                #[allow(clippy::cast_precision_loss)]
                let y = (PX_HEIGHT * mul) as f32 / height as f32;
                (x, y)
            };

            self.queue
                .write_buffer(&self.dim_uniform, 0, bytemuck::cast_slice(&[x, y]));

            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn on_lost(&mut self) {
        self.resize(self.size);
    }

    pub fn update(&mut self, rgba: &[u8]) {
        self.texture.update(&self.queue, rgba);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        self.queue.submit(core::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Texture {
    fn new(device: &wgpu::Device, width: u32, height: u32, label: Option<&str>) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view }
    }

    fn update(&mut self, queue: &wgpu::Queue, rgba: &[u8]) {
        use core::num::NonZeroU32;

        queue.write_texture(
            wgpu::ImageCopyTexture {
                aspect: wgpu::TextureAspect::All,
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * self.texture.width()),
                rows_per_image: NonZeroU32::new(self.texture.height()),
            },
            self.texture.size(),
        );
    }
}
