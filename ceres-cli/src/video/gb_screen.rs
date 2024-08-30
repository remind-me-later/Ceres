use super::texture::Texture;
use crate::{Scaling, PX_HEIGHT, PX_WIDTH};
use wgpu::util::DeviceExt;

pub(super) struct GBScreen {
    render_pipeline: wgpu::RenderPipeline,

    // Shader config binds
    dimensions_uniform: wgpu::Buffer,
    scale_uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // Texture binds
    texture: Texture,
    diffuse_bind_group: wgpu::BindGroup,
}

impl GBScreen {
    #[allow(clippy::too_many_lines)]
    pub(super) fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        scaling: Scaling,
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
            contents: bytemuck::cast_slice(&[scaling as u32]),
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
            device.create_shader_module(wgpu::include_wgsl!("../../shader/gb_screen.wgsl"));

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
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

    pub(super) fn update_screen_texture(&mut self, queue: &wgpu::Queue, rgb: &[u8]) {
        // TODO: awful way of transforming rgb to rgba
        let rgba = {
            const BUFFER_SIZE: usize = (PX_HEIGHT * PX_WIDTH * 4) as usize;
            let mut rgba: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];

            let mut j = 0;

            rgb.chunks_exact(3).for_each(|p| {
                rgba[j] = p[0];
                rgba[j + 1] = p[1];
                rgba[j + 2] = p[2];
                // Ignore alpha channel since we set composition mode to opaque
                j += 4;
            });

            rgba
        };

        self.texture.update(queue, &rgba);
    }

    pub(super) fn scale(&mut self, queue: &wgpu::Queue, scaling: Scaling) {
        queue.write_buffer(
            &self.scale_uniform,
            0,
            bytemuck::cast_slice(&[scaling as u32]),
        );
    }

    pub(super) fn resize(&mut self, queue: &wgpu::Queue, new_size: winit::dpi::PhysicalSize<u32>) {
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

        queue.write_buffer(&self.dimensions_uniform, 0, bytemuck::cast_slice(&[x, y]));
    }

    pub(super) fn render(&mut self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
        render_pass.set_bind_group(1, &self.uniform_bind_group, &[]);
        render_pass.draw(0..4, 0..1);
    }
}
