use ceres_std::wgpu_renderer::wgpu;
use ceres_std::{ShaderOption, wgpu_renderer::PipelineWrapper};
use jni::sys::{JNIEnv, jobject};
use ndk::native_window::NativeWindow;
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, DisplayHandle, HandleError, HasDisplayHandle,
    HasWindowHandle, RawDisplayHandle, RawWindowHandle, WindowHandle,
};
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct NativeWindowWrapper {
    window: Arc<Mutex<NativeWindow>>,
}

impl HasWindowHandle for NativeWindowWrapper {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        unsafe {
            let a_native_window = self.window.lock().unwrap();
            let handle = AndroidNdkWindowHandle::new(
                core::ptr::NonNull::new(a_native_window.ptr().as_ptr() as *mut c_void).unwrap(),
            );
            Ok(WindowHandle::borrow_raw(RawWindowHandle::AndroidNdk(
                handle,
            )))
        }
    }
}

impl HasDisplayHandle for NativeWindowWrapper {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        unsafe {
            Ok(DisplayHandle::borrow_raw(RawDisplayHandle::Android(
                AndroidDisplayHandle::new(),
            )))
        }
    }
}

pub struct State {
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    gb_screen: PipelineWrapper<{ ceres_std::PX_WIDTH as u32 }, { ceres_std::PX_HEIGHT as u32 }>,
    new_shader_option: Option<ShaderOption>,
    new_size: Option<(u32, u32)>,
    pixel_perfect: bool,
    queue: wgpu::Queue,
    size: (u32, u32),
    surface: wgpu::Surface<'static>,
    native_window_wrapper: NativeWindowWrapper,
}

impl State {
    pub async fn new(
        env: *mut JNIEnv,
        surface: jobject,
        shader_option: ShaderOption,
        pixel_perfect: bool,
    ) -> anyhow::Result<Self> {
        use anyhow::Context;

        let native_window = unsafe { NativeWindow::from_surface(env, surface).unwrap() };

        let size = (native_window.width() as u32, native_window.height() as u32);
        let native_window_wrapper = NativeWindowWrapper {
            window: Arc::new(Mutex::new(native_window)),
        };
        let instance = wgpu::Instance::default();

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = instance.create_surface(native_window_wrapper.clone())?;

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

        // let surface_caps = surface.get_capabilities(&adapter);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8Unorm,
            width: size.0,
            height: size.1,
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
            native_window_wrapper,
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

    pub const fn resize(&mut self, new_size: (u32, u32)) {
        self.new_size = Some(new_size);
    }

    pub fn update_texture(&mut self, rgba: &[u8]) {
        self.gb_screen
            .update_screen_texture(&self.device, &self.queue, rgba);
    }
}
