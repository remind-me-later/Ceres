use std::sync::{Arc, Mutex};

use crate::ShaderOption;
use ceres_std::Gb;
use eframe::egui;
use eframe::wgpu;
use eframe::wgpu::util::DeviceExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PixelMode {
    PixelPerfect,
    #[default]
    FitWindow,
}

pub struct Resources {
    render_pipeline: wgpu::RenderPipeline,

    // Shader config binds
    dimensions_uniform: wgpu::Buffer,
    scale_uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // Texture binds
    frame_texture: Texture,
    prev_frame_texture: Texture,
    diffuse_bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,

    // Bind group layout
    texture_bind_group_layout: wgpu::BindGroupLayout,
}

pub struct GBScreen<const PX_WIDTH: u32, const PX_HEIGHT: u32> {
    gb: Arc<Mutex<Gb>>,
    shader_option: ShaderOption,
    pixel_mode: PixelMode,
    size: (f32, f32),
}

impl<const PX_WIDTH: u32, const PX_HEIGHT: u32> GBScreen<PX_WIDTH, PX_HEIGHT> {
    #[expect(clippy::too_many_lines)]
    pub fn new<'a>(
        cc: &'a eframe::CreationContext<'a>,
        gb: Arc<Mutex<Gb>>,
        shader_option: ShaderOption,
    ) -> Self {
        // Get the WGPU render state from the eframe creation context. This can also be retrieved
        // from `eframe::Frame` when you don't have a `CreationContext` available.
        if let Some(wgpu_render_state) = cc.wgpu_render_state.as_ref() {
            let device = &wgpu_render_state.device;

            let frame_texture = Texture::new(device, PX_WIDTH, PX_HEIGHT, Some("current_frame"));
            let prev_frame_texture = Texture::new(device, PX_WIDTH, PX_HEIGHT, Some("prev_frame"));

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
                        resource: wgpu::BindingResource::TextureView(frame_texture.view()),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(prev_frame_texture.view()),
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
                device.create_shader_module(wgpu::include_wgsl!("../shader/gb_screen.wgsl"));

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
                        format: wgpu_render_state.target_format,
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
                    frame_texture,
                    diffuse_bind_group,
                    prev_frame_texture,
                    texture_bind_group_layout,
                    sampler,
                });
        }

        Self {
            gb,
            shader_option,
            size: (0.0, 0.0),
            pixel_mode: PixelMode::default(),
        }
    }

    pub fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
            response.rect,
            Self {
                gb: Arc::clone(&self.gb),
                shader_option: self.shader_option,
                size: (response.rect.width(), response.rect.height()),
                pixel_mode: self.pixel_mode,
            },
        ));
    }

    pub fn shader_option(&self) -> ShaderOption {
        self.shader_option
    }

    pub fn shader_option_mut(&mut self) -> &mut ShaderOption {
        &mut self.shader_option
    }

    pub fn pixel_mode(&self) -> PixelMode {
        self.pixel_mode
    }

    pub fn mut_pixel_mode(&mut self) -> &mut PixelMode {
        &mut self.pixel_mode
    }
}

impl<const PX_WIDTH: u32, const PX_HEIGHT: u32> eframe::egui_wgpu::CallbackTrait
    for GBScreen<PX_WIDTH, PX_HEIGHT>
{
    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &eframe::egui_wgpu::CallbackResources,
    ) {
        if let Some(resources) = callback_resources.get::<Resources>() {
            render_pass.set_pipeline(&resources.render_pipeline);
            render_pass.set_bind_group(0, &resources.diffuse_bind_group, &[]);
            render_pass.set_bind_group(1, &resources.uniform_bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        } else {
            eprintln!("No resources found for GBScreen");
        }
    }

    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &eframe::egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut eframe::egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(resources) = callback_resources.get_mut::<Resources>() {
            if let Ok(gb) = self.gb.lock() {
                // Swap the frame textures
                std::mem::swap(
                    &mut resources.frame_texture,
                    &mut resources.prev_frame_texture,
                );

                // Update the bind group with the new texture views
                resources.diffuse_bind_group =
                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &resources.texture_bind_group_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(
                                    resources.frame_texture.view(),
                                ),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&resources.sampler),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(
                                    resources.prev_frame_texture.view(),
                                ),
                            },
                        ],
                        label: None,
                    });

                // Update the current frame texture
                resources.frame_texture.update(queue, gb.pixel_data_rgba());
            }

            {
                let width = self.size.0;
                let height = self.size.1;
                #[expect(
                    clippy::cast_precision_loss,
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss
                )]
                let (x, y) = if matches!(self.pixel_mode, PixelMode::PixelPerfect) {
                    let mul = (width / PX_WIDTH as f32)
                        .min(height / PX_HEIGHT as f32)
                        .floor() as u32;
                    let x = (PX_WIDTH * mul) as f32 / width;
                    let y = (PX_HEIGHT * mul) as f32 / height;
                    (x, y)
                } else {
                    let mul = (width / PX_WIDTH as f32).min(height / PX_HEIGHT as f32);
                    let x = (PX_WIDTH as f32 * mul) / width;
                    let y = (PX_HEIGHT as f32 * mul) / height;
                    (x, y)
                };

                queue.write_buffer(
                    &resources.dimensions_uniform,
                    0,
                    bytemuck::cast_slice(&[x, y]),
                );
            }
            {
                queue.write_buffer(
                    &resources.scale_uniform,
                    0,
                    bytemuck::cast_slice(&[self.shader_option as u32]),
                );
            }
        } else {
            eprintln!("No resources found for GBScreen");
        }

        Vec::with_capacity(0)
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
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * self.texture.width()),
                rows_per_image: Some(self.texture.height()),
            },
            self.texture.size(),
        );
    }
}
