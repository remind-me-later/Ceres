use ceres_std::wgpu_renderer::wgpu;
use ceres_std::{ShaderOption, wgpu_renderer::PipelineWrapper};
use jni::JNIEnv;
use jni::objects::JObject;
use ndk::native_window::NativeWindow;
use raw_window_handle::{AndroidDisplayHandle, HasWindowHandle, RawDisplayHandle};

pub struct State {
    _native_window: NativeWindow,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    gb_screen: PipelineWrapper<{ ceres_std::PX_WIDTH as u32 }, { ceres_std::PX_HEIGHT as u32 }>,
    new_shader_option: Option<ShaderOption>,
    new_size: Option<(u32, u32)>,
    pixel_perfect: bool,
    queue: wgpu::Queue,
    size: (u32, u32),
    surface: wgpu::Surface<'static>,
}

impl State {
    pub async fn new(
        env: JNIEnv<'_>,
        surface: JObject<'_>,
        shader_option: ShaderOption,
        pixel_perfect: bool,
    ) -> anyhow::Result<Self> {
        use anyhow::Context;

        // Validate inputs
        if surface.is_null() {
            return Err(anyhow::anyhow!("Surface object is null"));
        }

        let native_window = unsafe {
            NativeWindow::from_surface(env.get_raw(), surface.as_raw())
                .context("Failed to create NativeWindow from surface")?
        };

        #[expect(clippy::cast_sign_loss)]
        let size = (native_window.width() as u32, native_window.height() as u32);

        // Validate window size
        if size.0 == 0 || size.1 == 0 {
            return Err(anyhow::anyhow!(
                "Invalid window size: {}x{}",
                size.0,
                size.1
            ));
        }

        let instance = wgpu::Instance::default();

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let wgpu_surface = unsafe {
            let window_handle = native_window
                .window_handle()
                .context("Failed to get window handle")?;
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: RawDisplayHandle::Android(AndroidDisplayHandle::new()),
                    raw_window_handle: window_handle.as_raw(),
                })
                .context("Failed to create wgpu surface")?
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&wgpu_surface),
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

        let surface_caps = wgpu_surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .first()
            .copied()
            .context("No supported surface formats found")?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.0,
            height: size.1,
            present_mode: surface_caps
                .present_modes
                .iter()
                .copied()
                .find(|m| *m == wgpu::PresentMode::AutoVsync)
                .unwrap_or(surface_caps.present_modes[0]),
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };

        wgpu_surface.configure(&device, &config);

        let gb_screen_renderer = PipelineWrapper::new(&device, config.format, shader_option.into());

        Ok(Self {
            surface: wgpu_surface,
            device,
            queue,
            _native_window: native_window,
            config,
            size,
            gb_screen: gb_screen_renderer,
            new_size: None,
            new_shader_option: None,
            pixel_perfect,
        })
    }

    pub const fn on_lost(&mut self) {
        self.resize(self.size.0, self.size.1);
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        if let Some(scaling) = self.new_shader_option.take() {
            self.gb_screen.shader_option(&self.queue, scaling.into());
        }

        if let Some(new_size) = self.new_size.take() {
            self.gb_screen
                .resize(self.pixel_perfect, &self.queue, new_size.0, new_size.1);
            self.size = new_size;
            self.config.width = new_size.0;
            self.config.height = new_size.1;
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

    pub const fn resize(&mut self, width: u32, height: u32) {
        self.new_size = Some((width, height));
    }

    pub const fn set_shader_option(&mut self, shader_option: ShaderOption) {
        self.new_shader_option = Some(shader_option);
    }

    pub fn update_texture(&mut self, rgba: &[u8]) {
        self.gb_screen
            .update_screen_texture(&self.device, &self.queue, rgba);
    }
}
