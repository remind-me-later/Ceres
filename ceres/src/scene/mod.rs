mod pipeline;
mod texture;

use std::sync::{Arc, Mutex};

use ceres_core::{Button, Gb};
use iced::{event, keyboard::Key, mouse, widget::shader, Rectangle};
use pipeline::Pipeline;

use crate::{audio, Scaling, PX_HEIGHT, PX_WIDTH};

pub struct Scene {
    gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
    scaling: Scaling,
}

impl Scene {
    pub fn new(gb: Arc<Mutex<Gb<audio::RingBuffer>>>, scaling: Scaling) -> Self {
        Self { gb, scaling }
    }

    pub fn set_scaling(&mut self, scaling: Scaling) {
        self.scaling = scaling;
    }

    pub fn scaling(&self) -> Scaling {
        self.scaling()
    }
}

impl<Message> shader::Program<Message> for Scene {
    type State = ();
    type Primitive = Primitive;

    fn draw(
        &self,
        _state: &Self::State,
        _cursor: mouse::Cursor,
        _bounds: Rectangle,
    ) -> Self::Primitive {
        let gb = self.gb.lock().unwrap();

        Primitive::new(&gb, self.scaling)
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: shader::Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
        _shell: &mut iced::advanced::Shell<'_, Message>,
    ) -> (event::Status, Option<Message>) {
        match event {
            shader::Event::Keyboard(e) => match e {
                iced::keyboard::Event::KeyPressed { key, .. } => {
                    let mut gb = self.gb.lock().unwrap();

                    match key {
                        Key::Character(c) => {
                            // gb.press(Button::Up);
                            match c.as_ref() {
                                "w" => gb.press(Button::Up),
                                "a" => gb.press(Button::Left),
                                "s" => gb.press(Button::Down),
                                "d" => gb.press(Button::Right),
                                "l" => gb.press(Button::A),
                                "k" => gb.press(Button::B),
                                "n" => gb.press(Button::Select),
                                "m" => gb.press(Button::Start),
                                _ => return (event::Status::Ignored, None),
                            }

                            return (event::Status::Captured, None);
                        }
                        _ => {}
                    }
                }
                iced::keyboard::Event::KeyReleased { key, .. } => {
                    let mut gb = self.gb.lock().unwrap();

                    match key {
                        Key::Character(c) => {
                            // gb.press(Button::Up);
                            match c.as_ref() {
                                "w" => gb.release(Button::Up),
                                "a" => gb.release(Button::Left),
                                "s" => gb.release(Button::Down),
                                "d" => gb.release(Button::Right),
                                "l" => gb.release(Button::A),
                                "k" => gb.release(Button::B),
                                "n" => gb.release(Button::Select),
                                "m" => gb.release(Button::Start),
                                _ => return (event::Status::Ignored, None),
                            }

                            return (event::Status::Captured, None);
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            _ => {}
        }

        (event::Status::Ignored, None)
    }
}

#[derive(Debug)]
pub struct Primitive {
    rgb: [u8; PX_HEIGHT as usize * PX_WIDTH as usize * 3],
    scaling: Scaling,
}

impl Primitive {
    pub fn new(gb: &Gb<audio::RingBuffer>, scaling: Scaling) -> Self {
        let mut rgb = [0; PX_HEIGHT as usize * PX_WIDTH as usize * 3];

        rgb.copy_from_slice(gb.pixel_data_rgb());

        Self { rgb, scaling }
    }
}

impl shader::Primitive for Primitive {
    fn prepare(
        &self,
        format: shader::wgpu::TextureFormat,
        device: &shader::wgpu::Device,
        queue: &shader::wgpu::Queue,
        _bounds: Rectangle,
        target_size: iced::Size<u32>,
        _scale_factor: f32,
        storage: &mut shader::Storage,
    ) {
        if !storage.has::<Pipeline>() {
            storage.store(Pipeline::new(
                device,
                queue,
                format,
                target_size,
                self.scaling,
            ));
        }

        let pipeline = storage.get_mut::<Pipeline>().unwrap();

        // Upload data to GPU
        pipeline.update(device, queue, target_size, self.scaling, &self.rgb);
    }

    fn render(
        &self,
        storage: &shader::Storage,
        target: &shader::wgpu::TextureView,
        _target_size: iced::Size<u32>,
        viewport: Rectangle<u32>,
        encoder: &mut shader::wgpu::CommandEncoder,
    ) {
        // At this point our pipeline should always be initialized
        let pipeline = storage.get::<Pipeline>().unwrap();

        // Render primitive
        pipeline.render(encoder, target, viewport);
    }
}