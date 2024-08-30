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
