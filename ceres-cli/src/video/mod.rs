mod gb_screen;
mod texture;

use crate::Scaling;
use alloc::sync::Arc;
use gb_screen::GBScreen;

// const RGB_BUFFER_SIZE: usize = (3 * PX_WIDTH * PX_HEIGHT) as usize;

pub struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    new_size: Option<winit::dpi::PhysicalSize<u32>>,
    new_scaling: Option<Scaling>,

    gb_screen: GBScreen,

    // Make sure that the winit window is last in the struct so that
    // it is dropped after the wgpu surface is dropped, otherwise the
    // program may crash when closed. This is probably a bug in wgpu.
    window: Arc<winit::window::Window>,
}

impl<'a> State<'a> {
    pub async fn new(window: winit::window::Window, scaling: Scaling) -> anyhow::Result<Self> {
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
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await?;

        // let surface_caps = surface.get_capabilities(&adapter);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        surface.configure(&device, &config);

        let gb_screen_renderer = GBScreen::new(&device, &config, scaling);

        Ok(Self {
            surface,
            device,
            queue,
            window,
            config,
            size,
            gb_screen: gb_screen_renderer,
            new_size: None,
            new_scaling: None,
        })
    }

    pub const fn window(&self) -> &Arc<winit::window::Window> {
        &self.window
    }

    pub fn choose_scale_mode(&mut self, scaling: Scaling) {
        self.new_scaling = Some(scaling);
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.new_size = Some(new_size);
    }

    pub fn on_lost(&mut self) {
        self.resize(self.size);
    }

    pub fn update_texture(&mut self, rgba: &[u8]) {
        self.gb_screen.update_screen_texture(&self.queue, rgba);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if let Some(scaling) = self.new_scaling.take() {
            self.gb_screen.scale(&self.queue, scaling);
        }

        if let Some(new_size) = self.new_size.take() {
            self.gb_screen.resize(&self.queue, new_size);
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

                self.gb_screen.render(&mut render_pass);
            }

            self.queue.submit(core::iter::once(encoder.finish()));
        }

        output.present();

        Ok(())
    }
}