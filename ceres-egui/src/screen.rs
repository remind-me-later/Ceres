use ceres_std::ShaderOption;
use ceres_std::wgpu_renderer::PipelineWrapper;
use eframe::egui;
use eframe::wgpu;
use std::sync::Arc;
use std::sync::Mutex;

/// The last emulated frame, shared with the paint callback.
pub struct Frame {
    /// The emulated frame it is (`GbThread::frame`).
    pub number: u64,
    pub rgba: Box<[u8]>,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            number: 0,
            rgba: vec![0; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
        }
    }
}

/// The emulated frame the screen texture holds, kept with the pipeline.
struct UploadedFrame(u64);

pub struct GBScreen<const PX_WIDTH: u32, const PX_HEIGHT: u32> {
    buffer: Arc<Mutex<Frame>>,
    pixel_perfect: bool,
    shader_option: ShaderOption,
    size: (f32, f32),
}

impl<const PX_WIDTH: u32, const PX_HEIGHT: u32> GBScreen<PX_WIDTH, PX_HEIGHT> {
    pub fn custom_painting(&self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

        painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
            response.rect,
            Self {
                buffer: Arc::clone(&self.buffer),
                shader_option: self.shader_option,
                size: (response.rect.width(), response.rect.height()),
                pixel_perfect: self.pixel_perfect,
            },
        ));
    }

    pub const fn mut_buffer(&mut self) -> &mut Arc<Mutex<Frame>> {
        &mut self.buffer
    }

    pub const fn mut_pixel_perfect(&mut self) -> &mut bool {
        &mut self.pixel_perfect
    }

    pub fn new(
        cc: &eframe::CreationContext<'_>,
        gb: Arc<Mutex<Frame>>,
        shader_option: ShaderOption,
    ) -> Self {
        // Get the WGPU render state from the eframe creation context. This can also be retrieved
        // from `eframe::Frame` when you don't have a `CreationContext` available.
        if let Some(wgpu_render_state) = cc.wgpu_render_state.as_ref() {
            let device = &wgpu_render_state.device;

            let pipeline_wrapper = PipelineWrapper::<PX_WIDTH, PX_HEIGHT>::new(
                device,
                wgpu_render_state.target_format,
                shader_option.into(),
            );

            let resources = &mut wgpu_render_state.renderer.write().callback_resources;
            resources.insert(pipeline_wrapper);
            resources.insert(UploadedFrame(0));
        }

        Self {
            buffer: gb,
            shader_option,
            size: (0.0, 0.0),
            pixel_perfect: false,
        }
    }

    pub const fn pixel_perfect(&self) -> bool {
        self.pixel_perfect
    }

    pub const fn shader_option(&self) -> ShaderOption {
        self.shader_option
    }

    pub const fn shader_option_mut(&mut self) -> &mut ShaderOption {
        &mut self.shader_option
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
        if let Some(pipeline) = callback_resources.get::<PipelineWrapper<PX_WIDTH, PX_HEIGHT>>() {
            pipeline.paint(render_pass);
        } else {
            eprintln!("No resources found for GBScreen");
        }
    }

    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &eframe::egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut eframe::egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let uploaded = callback_resources
            .get::<UploadedFrame>()
            .map_or(0, |uploaded| uploaded.0);
        let mut new_frame = None;
        if let Some(pipeline) = callback_resources.get_mut::<PipelineWrapper<PX_WIDTH, PX_HEIGHT>>()
        {
            // egui repaints more often than frames come: only a new one is
            // uploaded (and the previous one kept for the LCD shader).
            if let Ok(frame) = self.buffer.lock()
                && frame.number != uploaded
            {
                pipeline.upload_frame(queue, &frame.rgba);
                new_frame = Some(frame.number);
            }

            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            pipeline.resize(
                self.pixel_perfect,
                queue,
                self.size.0 as u32,
                self.size.1 as u32,
            );

            pipeline.shader_option(queue, self.shader_option.into());
        } else {
            eprintln!("No resources found for GBScreen");
        }
        if let Some(number) = new_frame {
            callback_resources.insert(UploadedFrame(number));
        }

        Vec::with_capacity(0)
    }
}
