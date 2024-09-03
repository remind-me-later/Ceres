use std::sync::{Arc, Mutex};

use crate::{audio, Scaling, PX_HEIGHT, PX_WIDTH};
use ceres_core::Gb;
use eframe::egui;
use eframe::wgpu::util::DeviceExt;
use eframe::wgpu::{self};

pub struct Resources {
    render_pipeline: wgpu::RenderPipeline,

    // Shader config binds
    dimensions_uniform: wgpu::Buffer,
    scale_uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // Texture binds
    texture: Texture,
    diffuse_bind_group: wgpu::BindGroup,
}

pub struct GBScreen {
    gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
    scaling: Scaling,
    size: (u32, u32),
}

impl GBScreen {
    #[allow(clippy::too_many_lines)]
    pub fn new<'a>(
        cc: &'a eframe::CreationContext<'a>,
        gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
    ) -> Self {
        // Get the WGPU render state from the eframe creation context. This can also be retrieved
        // from `eframe::Frame` when you don't have a `CreationContext` available.
        let wgpu_render_state = cc.wgpu_render_state.as_ref().unwrap();

        let device = &wgpu_render_state.device;

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
            contents: bytemuck::cast_slice(&[Scaling::default() as u32]),
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

        let shader = device.create_shader_module(wgpu::include_wgsl!("../shader/gb_screen.wgsl"));

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            // cache: None,
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
                    format: wgpu_render_state.target_format.into(),
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

        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(Resources {
                render_pipeline,
                dimensions_uniform,
                scale_uniform,
                uniform_bind_group,
                texture,
                diffuse_bind_group,
            });

        Self {
            gb,
            scaling: Default::default(),
            size: (0, 0),
        }
    }

    pub fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        self.size = (rect.width() as u32, rect.height() as u32);

        ui.painter()
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                rect,
                Self {
                    gb: self.gb.clone(),
                    scaling: self.scaling,
                    size: self.size,
                },
            ));
    }
}

impl eframe::egui_wgpu::CallbackTrait for GBScreen {
    fn paint<'a>(
        &'a self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'a>,
        callback_resources: &'a eframe::egui_wgpu::CallbackResources,
    ) {
        let resources: &Resources = callback_resources.get().unwrap();

        render_pass.set_pipeline(&resources.render_pipeline);
        render_pass.set_bind_group(0, &resources.diffuse_bind_group, &[]);
        render_pass.set_bind_group(1, &resources.uniform_bind_group, &[]);
        render_pass.draw(0..4, 0..1);
    }

    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &eframe::egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut eframe::egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources: &mut Resources = callback_resources.get_mut().unwrap();

        if let Ok(gb) = self.gb.lock() {
            // TODO: awful way of transforming rgb to rgba
            let rgba = {
                let rgb = gb.pixel_data_rgb();

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

            resources.texture.update(queue, &rgba);
        }

        {
            let width = self.size.0;
            let height = self.size.1;
            let (x, y) = {
                let mul = (width / PX_WIDTH).min(height / PX_HEIGHT);
                #[allow(clippy::cast_precision_loss)]
                let x = (PX_WIDTH * mul) as f32 / width as f32;
                #[allow(clippy::cast_precision_loss)]
                let y = (PX_HEIGHT * mul) as f32 / height as f32;
                (x, y)
            };

            queue.write_buffer(
                &resources.dimensions_uniform,
                0,
                bytemuck::cast_slice(&[x, y]),
            );
        }
        {
            let scaling = self.scaling;
            queue.write_buffer(
                &resources.scale_uniform,
                0,
                bytemuck::cast_slice(&[scaling as u32]),
            );
        }

        Vec::new()
    }
}

pub(super) struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl Texture {
    pub(super) fn new(device: &wgpu::Device, width: u32, height: u32, label: Option<&str>) -> Self {
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

    pub(super) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub(super) fn update(&mut self, queue: &wgpu::Queue, rgba: &[u8]) {
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
                bytes_per_row: Some(4 * self.texture.width()),
                rows_per_image: Some(self.texture.height()),
            },
            self.texture.size(),
        );
    }
}
