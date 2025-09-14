use ceres_std::wgpu_renderer::wgpu;
use ceres_std::{cli::ShaderOption, wgpu_renderer::PipelineWrapper};
use std::sync::Arc;

pub struct State<'a> {
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    gb_screen: PipelineWrapper<{ ceres_std::PX_WIDTH as u32 }, { ceres_std::PX_HEIGHT as u32 }>,
    new_shader_option: Option<ShaderOption>,
    new_size: Option<winit::dpi::PhysicalSize<u32>>,
    pixel_perfect: bool,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface<'a>,
    window: Arc<winit::window::Window>,
}

impl State<'_> {
    pub async fn new(
        window: winit::window::Window,
        shader_option: ShaderOption,
        pixel_perfect: bool,
    ) -> anyhow::Result<Self> {
        use anyhow::Context;

        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let window = Arc::new(window);

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = instance.create_surface(Arc::clone(&window))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("unable to obtain wgpu adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                trace: wgpu::Trace::Off,
            })
            .await?;

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .first()
            .copied()
            .context("no supported surface formats")?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        surface.configure(&device, &config);

        let gb_screen_renderer = PipelineWrapper::new(&device, config.format, shader_option.into());

        Ok(Self {
            surface,
            device,
            queue,
            window,
            config,
            size,
            gb_screen: gb_screen_renderer,
            new_size: None,
            new_shader_option: None,
            pixel_perfect,
        })
    }

    pub const fn on_lost(&mut self) {
        self.resize(self.size);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if let Some(scaling) = self.new_shader_option.take() {
            self.gb_screen.shader_option(&self.queue, scaling.into());
        }

        if let Some(new_size) = self.new_size.take() {
            self.gb_screen.resize(
                self.pixel_perfect,
                &self.queue,
                new_size.width,
                new_size.height,
            );
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }

        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
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
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });

                self.gb_screen.paint(&mut render_pass);
            }

            self.queue.submit(core::iter::once(encoder.finish()));
        }

        output.present();

        Ok(())
    }

    pub const fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.new_size = Some(new_size);
    }

    pub fn update_texture(&mut self, rgba: &[u8]) {
        self.gb_screen
            .update_screen_texture(&self.device, &self.queue, rgba);
    }

    pub const fn window(&self) -> &Arc<winit::window::Window> {
        &self.window
    }
}
