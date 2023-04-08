#![no_std]

use alloc::{boxed::Box, vec::Vec};
use ceres_core::{Button, Cart, Gb, Model};
use core::iter;
use either::Either;
use wasm_bindgen::prelude::*;

extern crate alloc;

#[wasm_bindgen]
pub struct Emulator {
    gb: Gb,
}

#[wasm_bindgen]
pub fn init_emulator() -> *mut Emulator {
    let model = Model::Cgb;
    let sample_rate = 48000;

    let cart = Cart::default();
    let emulator = Box::new(Emulator {
        gb: Gb::new(model, sample_rate, cart),
    });

    Box::leak(emulator)
}

#[wasm_bindgen]
pub fn init_emulator_with_rom(rom: Vec<u8>) -> *mut Emulator {
    let model = Model::Cgb;
    let sample_rate = 48000;

    // TODO: Handle errors more gracefully
    let cart = Cart::new(rom.into_boxed_slice(), None).unwrap_or_default();

    let emulator = Box::new(Emulator {
        gb: Gb::new(model, sample_rate, cart),
    });

    Box::leak(emulator)
}

#[wasm_bindgen]
pub unsafe fn destroy_emulator(emulator: *mut Emulator) {
    drop(Box::from_raw(emulator));
}

// TODO: Maybe avoid the copy into a Vec<u8>?
#[wasm_bindgen]
pub unsafe fn get_framebuffer(emulator: *const Emulator) -> Vec<u8> {
    // We need to add the alpha (255 value)
    (*emulator)
        .gb
        .pixel_data_rgba()
        .iter()
        .copied()
        .enumerate()
        .flat_map(|(i, c)| {
            if i % 3 == 0 {
                // Either::Left([255u8, c].iter())
                Either::Left(iter::once(255u8).chain(iter::once(c)))
            } else {
                Either::Right(iter::once(c))
            }
        })
        .chain(iter::once(255u8))
        .skip(1)
        .collect()
}

#[wasm_bindgen]
pub struct AudioSamples {
    pub left: i16,
    pub right: i16,
}

#[wasm_bindgen]
pub unsafe fn run_sample(emulator: *mut Emulator) -> AudioSamples {
    let (a, b) = (*emulator).gb.run_samples();
    AudioSamples { left: a, right: b }
}

#[wasm_bindgen]
pub unsafe fn run_n_samples(emulator: *mut Emulator, num_samples: i32) {
    for _ in 0..num_samples {
        (*emulator).gb.run_samples();
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
        _ => unreachable!(),
    }
}

// The button index is the same as Specified in Button enum
#[wasm_bindgen]
pub unsafe fn press_button(emulator: *mut Emulator, button: u8) {
    (*emulator).gb.press(u8_to_button(button));
}

#[wasm_bindgen]
pub unsafe fn release_button(emulator: *mut Emulator, button: u8) {
    (*emulator).gb.release(u8_to_button(button));
}
