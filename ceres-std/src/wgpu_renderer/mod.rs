mod texture;

pub use wgpu;

use texture::Texture;
use wgpu::util::DeviceExt as _;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ShaderOption {
    Crt = 4,
    Lcd = 3,
    #[default]
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
}

impl From<crate::ShaderOption> for ShaderOption {
    #[inline]
    fn from(shader_option: crate::ShaderOption) -> Self {
        match shader_option {
            crate::ShaderOption::Nearest => Self::Nearest,
            crate::ShaderOption::Scale2x => Self::Scale2x,
            crate::ShaderOption::Scale3x => Self::Scale3x,
            crate::ShaderOption::Lcd => Self::Lcd,
            crate::ShaderOption::Crt => Self::Crt,
        }
    }
}

/// Draws the Game Boy screen.
///
/// The current and the previous frame (the LCD shader blends the previous one
/// in) are two textures that swap roles on each new frame: a new frame is
/// written over the older one, and one of two bind groups made up front picks
/// which is which.
pub struct PipelineWrapper<const PX_WIDTH: u32, const PX_HEIGHT: u32> {
    /// `frame_bind_groups[i]` binds `textures[i]` as the current frame and
    /// the other one as the previous frame.
    frame_bind_groups: [wgpu::BindGroup; 2],
    /// The texture holding the current frame.
    current: usize,
    dimensions_uniform: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
    scale_uniform: wgpu::Buffer,
    textures: [Texture; 2],
    uniform_bind_group: wgpu::BindGroup,
}

impl<const PX_WIDTH: u32, const PX_HEIGHT: u32> PipelineWrapper<PX_WIDTH, PX_HEIGHT> {
    #[expect(
        clippy::too_many_lines,
        reason = "Initialization logic is long but necessary"
    )]
    #[must_use]
    #[inline]
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        shader_option: ShaderOption,
    ) -> Self {
        let textures = [
            Texture::new(device, PX_WIDTH, PX_HEIGHT, Some("frame_0")),
            Texture::new(device, PX_WIDTH, PX_HEIGHT, Some("frame_1")),
        ];

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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        },
                        count: None,
                    },
                ],
                label: None,
            });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        let frame_bind_groups: [wgpu::BindGroup; 2] = core::array::from_fn(|current| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(textures[current].view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(textures[1 - current].view()),
                    },
                ],
                label: None,
            })
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

        let dimensions_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let scale_uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&[shader_option as u32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: dimensions_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scale_uniform.as_entire_binding(),
                },
            ],
            label: None,
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    Some(&texture_bind_group_layout),
                    Some(&uniform_bind_group_layout),
                ],
                immediate_size: 0,
            });

        let shader =
            device.create_shader_module(wgpu::include_wgsl!("../../shader/gb_screen.wgsl"));

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
        });

        Self {
            frame_bind_groups,
            current: 0,
            dimensions_uniform,
            render_pipeline,
            scale_uniform,
            textures,
            uniform_bind_group,
        }
    }

    #[inline]
    pub fn paint(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.frame_bind_groups[self.current], &[]);
        render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
        render_pass.draw(0..4, 0..1);
    }

    #[inline]
    pub fn resize(&mut self, pixel_perfect: bool, queue: &wgpu::Queue, width: u32, height: u32) {
        let (x, y) = {
            #[expect(
                clippy::cast_precision_loss,
                reason = "PX_WIDTH and PX_HEIGHT are u8 constants, so this is safe"
            )]
            {
                let mul = if pixel_perfect {
                    (width / PX_WIDTH).min(height / PX_HEIGHT) as f32
                } else {
                    (width as f32 / PX_WIDTH as f32).min(height as f32 / PX_HEIGHT as f32)
                };

                let x = (PX_WIDTH as f32 * mul) / width as f32;
                let y = (PX_HEIGHT as f32 * mul) / height as f32;
                (x, y)
            }
        };

        #[expect(clippy::tuple_array_conversions)]
        queue.write_buffer(&self.dimensions_uniform, 0, bytemuck::cast_slice(&[x, y]));
    }

    #[inline]
    pub fn shader_option(&mut self, queue: &wgpu::Queue, shader_option: ShaderOption) {
        queue.write_buffer(
            &self.scale_uniform,
            0,
            bytemuck::cast_slice(&[shader_option as u32]),
        );
    }

    /// Shows a new frame: it goes over the older texture, and the current
    /// one becomes the previous one.
    #[inline]
    pub fn upload_frame(&mut self, queue: &wgpu::Queue, rgba: &[u8]) {
        self.current = 1 - self.current;
        self.textures[self.current].update(queue, rgba);
    }
}
