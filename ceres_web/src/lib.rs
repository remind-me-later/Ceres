mod utils;

use wasm_bindgen::prelude::*;
use ceres_core::{Button, Cart, Gb, Model};
use either::Either;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
extern {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(s: &str) {
    alert(&format!("Hello, {}!", s));
}

#[wasm_bindgen]
pub struct Emulator {
    emulator: Gb
}

#[wasm_bindgen]
pub fn init_emulator() -> *mut Emulator {
    let model = Model::Cgb;
    let sample_rate = 48000;

    let cart = Cart::default();
    let emulator = Box::new( Emulator {
        emulator: Gb::new(model, sample_rate, cart)
    });

    Box::leak(emulator)
}

#[wasm_bindgen]
pub fn init_emulator_with_rom(rom: Vec<u8>) -> *mut Emulator {
    let model = Model::Cgb;
    let sample_rate = 48000;

    // TODO: Handle errors more gracefully
    let cart = Cart::new(rom.into_boxed_slice(), None).unwrap();

    let emulator = Box::new( Emulator {
        emulator: Gb::new(model, sample_rate, cart)
    });

    Box::leak(emulator)
}

#[wasm_bindgen]
pub fn destroy_emulator(emulator: *mut Emulator) {
    unsafe {
        drop(Box::from_raw(emulator));
    }
}

// TODO: Maybe avoid the copy into a Vec<u8>?
#[wasm_bindgen]
pub fn get_framebuffer(emulator: *const Emulator) -> Vec<u8> {
    unsafe {
        // We need to add the alpha (255 value)
        (*emulator).emulator.pixel_data_rgba().iter()
            .copied()
            .enumerate()
            .flat_map(|(i, c)| {
                if i % 3 == 0 {
                    // Either::Left([255u8, c].iter())
                    Either::Left(
                        std::iter::once(255u8).chain(
                            std::iter::once(c)
                        )
                    )
                } else {
                    Either::Right(std::iter::once(c))
                }
            }).chain(std::iter::once(255u8)).skip(1).collect()
    }
}

#[wasm_bindgen]
pub struct AudioSamples {
    pub left: i16,
    pub right: i16
}

#[wasm_bindgen]
pub fn run_sample(emulator: *mut Emulator) -> AudioSamples {
    unsafe {
        let (a, b) = (*emulator).emulator.run_samples();
        AudioSamples {
            left: a,
            right: b
        }
    }
}

fn u8_to_button(value: u8) -> Button {
    match value {
        0x01 => Button::Right,
        0x02 => Button::Left,
        0x04 => Button::Up,
        0x08 => Button::Down,
        0x10 => Button::A,
        0x20 => Button::B,
        0x40 => Button::Select,
        0x80 => Button::Start,

        _ => unreachable!()
    }
}

// The button index is the same as Specified in Button enum
#[wasm_bindgen]
pub fn press_button(emulator: *mut Emulator, button: u8) {
    unsafe {
        (*emulator).emulator.press(u8_to_button(button));
    }
}

#[wasm_bindgen]
pub fn release_button(emulator: *mut Emulator, button: u8) {
    unsafe {
        (*emulator).emulator.release(u8_to_button(button));
    }
}