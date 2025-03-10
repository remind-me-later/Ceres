use crate::{ScalingOption, ShaderOption};

use super::texture::Texture;
use wgpu::util::DeviceExt;

pub(super) struct PipelineWrapper<const PX_WIDTH: u32, const PX_HEIGHT: u32> {
    render_pipeline: wgpu::RenderPipeline,

    // Shader config binds
    dimensions_uniform: wgpu::Buffer,
    scale_uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // Texture binds
    texture: Texture,
    diffuse_bind_group: wgpu::BindGroup,
}

impl<const PX_WIDTH: u32, const PX_HEIGHT: u32> PipelineWrapper<PX_WIDTH, PX_HEIGHT> {
    #[allow(clippy::too_many_lines)]
    pub(super) fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        shader_option: ShaderOption,
    ) -> Self {
        let texture = Texture::new(device, PX_WIDTH, PX_HEIGHT, None);

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

        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture.view()),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(texture.view()),
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
                bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let shader =
            device.create_shader_module(wgpu::include_wgsl!("../../../shader/gb_screen.wgsl"));

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
                    format: config.format,
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
            multiview: None,
        });

        Self {
            render_pipeline,
            dimensions_uniform,
            scale_uniform,
            uniform_bind_group,
            texture,
            diffuse_bind_group,
        }
    }

    pub(super) fn update_screen_texture(&mut self, queue: &wgpu::Queue, rgba: &[u8]) {
        self.texture.update(queue, rgba);
    }

    pub(super) fn shader_option(&mut self, queue: &wgpu::Queue, shader_option: ShaderOption) {
        queue.write_buffer(
            &self.scale_uniform,
            0,
            bytemuck::cast_slice(&[shader_option as u32]),
        );
    }

    pub(super) fn resize(
        &mut self,
        scaling_option: ScalingOption,
        queue: &wgpu::Queue,
        new_size: winit::dpi::PhysicalSize<u32>,
    ) {
        let width = new_size.width;
        let height = new_size.height;

        let (x, y) = if matches!(scaling_option, ScalingOption::PixelPerfect) {
            let mul = (width / PX_WIDTH).min(height / PX_HEIGHT);
            #[allow(clippy::cast_precision_loss)]
            let x = (PX_WIDTH * mul) as f32 / width as f32;
            #[allow(clippy::cast_precision_loss)]
            let y = (PX_HEIGHT * mul) as f32 / height as f32;
            (x, y)
        } else {
            #[allow(clippy::cast_precision_loss)]
            let mul = (width as f32 / PX_WIDTH as f32).min(height as f32 / PX_HEIGHT as f32);
            #[allow(clippy::cast_precision_loss)]
            let x = (PX_WIDTH as f32 * mul) / width as f32;
            #[allow(clippy::cast_precision_loss)]
            let y = (PX_HEIGHT as f32 * mul) / height as f32;
            (x, y)
        };

        queue.write_buffer(&self.dimensions_uniform, 0, bytemuck::cast_slice(&[x, y]));
    }

    pub(super) fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
        render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
        render_pass.draw(0..4, 0..1);
    }
}
