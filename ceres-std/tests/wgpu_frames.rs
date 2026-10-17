//! The screen pipeline's frame textures: an upload shows the new frame and
//! keeps the one before it for the LCD shader's ghosting; a repaint without
//! an upload changes nothing. Renders offscreen; skipped without a GPU.
#![cfg(feature = "wgpu_renderer")]

use ceres_std::wgpu_renderer::{PipelineWrapper, ShaderOption, wgpu};

const W: u32 = 160;
const H: u32 = 144;
/// Rows padded to wgpu's copy alignment (256 bytes).
const BYTES_PER_ROW: u32 = 768;

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: wgpu::Texture,
    readback: wgpu::Buffer,
}

impl Gpu {
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: W,
                height: H,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: u64::from(BYTES_PER_ROW * H),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Some(Self {
            device,
            queue,
            target,
            readback,
        })
    }

    /// Paints the screen and returns its middle pixel.
    #[expect(clippy::unwrap_used, reason = "a test")]
    fn paint(&self, pipeline: &PipelineWrapper<W, H>) -> [u8; 4] {
        let view = self
            .target
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pipeline.paint(&mut pass);
        }
        encoder.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(BYTES_PER_ROW),
                    rows_per_image: Some(H),
                },
            },
            self.target.size(),
        );
        self.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let at = (H / 2 * BYTES_PER_ROW + W / 2 * 4) as usize;
        let pixel = {
            let data = slice.get_mapped_range().unwrap();
            [data[at], data[at + 1], data[at + 2], data[at + 3]]
        };
        self.readback.unmap();
        pixel
    }
}

/// A frame of one color.
fn solid(rgb: [u8; 3]) -> Vec<u8> {
    (0..W * H)
        .flat_map(|_| [rgb[0], rgb[1], rgb[2], 0xFF])
        .collect()
}

#[test]
fn shows_the_new_frame_and_ghosts_the_previous_one() {
    let Some(gpu) = Gpu::new() else {
        eprintln!("no GPU: skipped");
        return;
    };
    let mut pipeline = PipelineWrapper::<W, H>::new(
        &gpu.device,
        wgpu::TextureFormat::Rgba8Unorm,
        ShaderOption::Nearest,
    );
    pipeline.resize(false, &gpu.queue, W, H);

    pipeline.upload_frame(&gpu.queue, &solid([0xFF, 0, 0]));
    pipeline.upload_frame(&gpu.queue, &solid([0, 0, 0xFF]));
    assert_eq!(gpu.paint(&pipeline), [0, 0, 0xFF, 0xFF], "the new frame");

    // The LCD shader mixes in the previous frame (red under blue).
    pipeline.shader_option(&gpu.queue, ShaderOption::Lcd);
    let [red, green, blue, _] = gpu.paint(&pipeline);
    assert!(
        red > 0x40 && green == 0 && blue > 0x40,
        "red ghost under blue"
    );

    pipeline.upload_frame(&gpu.queue, &solid([0, 0xFF, 0]));
    let ghosted = gpu.paint(&pipeline);
    let [red2, green2, blue2, _] = ghosted;
    assert!(
        red2 == 0 && green2 > 0x40 && blue2 > 0x40,
        "blue ghost under green"
    );
    assert_eq!(gpu.paint(&pipeline), ghosted, "a repaint changes nothing");
}
