#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
  position:   [f32; 2],
  tex_coords: [f32; 2],
}

impl Vertex {
  #[allow(clippy::too_many_lines)]
  const fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
    use core::mem;
    wgpu::VertexBufferLayout {
      array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
      step_mode:    wgpu::VertexStepMode::Vertex,
      attributes:   &[
        wgpu::VertexAttribute {
          offset:          0,
          shader_location: 0,
          format:          wgpu::VertexFormat::Float32x3,
        },
        wgpu::VertexAttribute {
          offset:          mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
          shader_location: 1,
          format:          wgpu::VertexFormat::Float32x2,
        },
      ],
    }
  }
}

pub struct State {
  surface:            wgpu::Surface,
  device:             wgpu::Device,
  queue:              wgpu::Queue,
  config:             wgpu::SurfaceConfiguration,
  size:               winit::dpi::PhysicalSize<u32>,
  render_pipeline:    wgpu::RenderPipeline,
  vertex_buffer:      wgpu::Buffer,
  texture:            Texture,
  diffuse_bind_group: wgpu::BindGroup,
  window:             winit::window::Window,
}

impl State {
  #[allow(clippy::too_many_lines)]
  pub async fn new(window: winit::window::Window, width: u32, height: u32) -> anyhow::Result<Self> {
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
        power_preference:       wgpu::PowerPreference::LowPower,
        compatible_surface:     Some(&surface),
        force_fallback_adapter: false,
      })
      .await
      .context("unable to obtain wgpu adapter")?;

    let (device, queue) = adapter
      .request_device(
        &wgpu::DeviceDescriptor {
          label:    None,
          features: wgpu::Features::empty(),
          limits:   wgpu::Limits::downlevel_webgl2_defaults(),
        },
        None,
      )
      .await?;

    let surface_caps = surface.get_capabilities(&adapter);

    let config = wgpu::SurfaceConfiguration {
      usage:        wgpu::TextureUsages::RENDER_ATTACHMENT,
      format:       wgpu::TextureFormat::Bgra8Unorm,
      width:        size.width,
      height:       size.height,
      present_mode: surface_caps.present_modes[0],
      alpha_mode:   wgpu::CompositeAlphaMode::Auto,
      view_formats: vec![],
    };

    surface.configure(&device, &config);

    let texture = Texture::new(&device, width, height, None);

    let texture_bind_group_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
          wgpu::BindGroupLayoutEntry {
            binding:    0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty:         wgpu::BindingType::Texture {
              multisampled:   false,
              view_dimension: wgpu::TextureViewDimension::D2,
              sample_type:    wgpu::TextureSampleType::Float { filterable: false },
            },
            count:      None,
          },
          wgpu::BindGroupLayoutEntry {
            binding:    1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty:         wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
            count:      None,
          },
        ],
        label:   Some("texture_bind_group_layout"),
      });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

    let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout:  &texture_bind_group_layout,
      entries: &[
        wgpu::BindGroupEntry {
          binding:  0,
          resource: wgpu::BindingResource::TextureView(&texture.view),
        },
        wgpu::BindGroupEntry {
          binding:  1,
          resource: wgpu::BindingResource::Sampler(&sampler),
        },
      ],
      label:   Some("diffuse_bind_group"),
    });

    let shader = device.create_shader_module(wgpu::include_spirv!("../shader/near.spv"));

    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label:                Some("Render Pipeline Layout"),
      bind_group_layouts:   &[&texture_bind_group_layout],
      push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label:         Some("Render Pipeline"),
      layout:        Some(&render_pipeline_layout),
      vertex:        wgpu::VertexState {
        module:      &shader,
        entry_point: "vs_main",
        buffers:     &[Vertex::desc()],
      },
      fragment:      Some(wgpu::FragmentState {
        module:      &shader,
        entry_point: "fs_main",
        targets:     &[Some(wgpu::ColorTargetState {
          format:     config.format,
          blend:      Some(wgpu::BlendState::REPLACE),
          write_mask: wgpu::ColorWrites::ALL,
        })],
      }),
      primitive:     wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleStrip,
        ..Default::default()
      },
      depth_stencil: None,
      multisample:   wgpu::MultisampleState::default(),
      multiview:     None,
    });

    let vertices: &[Vertex] = &[
      Vertex {
        position:   [1.0, 1.0],
        tex_coords: [1.0, 1.0],
      },
      Vertex {
        position:   [-1.0, 1.0],
        tex_coords: [0.0, 1.0],
      },
      Vertex {
        position:   [1.0, -1.0],
        tex_coords: [1.0, 0.0],
      },
      Vertex {
        position:   [-1.0, -1.0],
        tex_coords: [0.0, 0.0],
      },
    ];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label:    Some("Vertex Buffer"),
      contents: bytemuck::cast_slice(vertices),
      usage:    wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });

    Ok(Self {
      surface,
      device,
      queue,
      config,
      size,
      render_pipeline,
      vertex_buffer,
      texture,
      diffuse_bind_group,
      window,
    })
  }

  pub const fn window(&self) -> &winit::window::Window {
    &self.window
  }

  pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
    if new_size.width > 0 && new_size.height > 0 {
      const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
      const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
      let width = new_size.width;
      let height = new_size.height;

      let (x, y) = {
        let mul = (width / PX_WIDTH).min(height / PX_HEIGHT);
        let x = (PX_WIDTH * mul) as f32 / width as f32;
        let y = (PX_HEIGHT * mul) as f32 / height as f32;
        (x, y)
      };

      let vertices = &[
        Vertex {
          position:   [x, y],
          tex_coords: [1.0, 1.0],
        },
        Vertex {
          position:   [-x, y],
          tex_coords: [0.0, 1.0],
        },
        Vertex {
          position:   [x, -y],
          tex_coords: [1.0, 0.0],
        },
        Vertex {
          position:   [-x, -y],
          tex_coords: [0.0, 0.0],
        },
      ];

      self
        .queue
        .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));

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
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
      });

    {
      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label:                    Some("Render Pass"),
        color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
          view:           &view,
          resolve_target: None,
          ops:            wgpu::Operations {
            load:  wgpu::LoadOp::Clear(wgpu::Color::BLACK),
            store: true,
          },
        })],
        depth_stencil_attachment: None,
      });

      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..4, 0..1);
    }

    self.queue.submit(core::iter::once(encoder.finish()));
    output.present();

    Ok(())
  }
}

struct Texture {
  texture: wgpu::Texture,
  view:    wgpu::TextureView,
}

impl Texture {
  fn new(device: &wgpu::Device, width: u32, height: u32, label: Option<&str>) -> Self {
    let size = wgpu::Extent3d {
      width,
      height,
      depth_or_array_layers: 1,
    };
    let format = wgpu::TextureFormat::Bgra8Unorm;
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
        aspect:    wgpu::TextureAspect::All,
        texture:   &self.texture,
        mip_level: 0,
        origin:    wgpu::Origin3d::ZERO,
      },
      rgba,
      wgpu::ImageDataLayout {
        offset:         0,
        bytes_per_row:  NonZeroU32::new(4 * self.texture.width()),
        rows_per_image: NonZeroU32::new(self.texture.height()),
      },
      self.texture.size(),
    );
  }
}
