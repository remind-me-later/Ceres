#![allow(non_snake_case)]

use crate::emulator::Emulator;
use ceres_std::Button;
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jfloat, jint, jlong};
use log::debug;
use std::path::PathBuf;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_createEmulator(
    env: JNIEnv,
    _class: JClass,
) -> jlong {
    let emulator = Box::new(Emulator::new());
    let ptr = Box::into_raw(emulator);
    debug!("Emulator created at {:p}", ptr);
    ptr as jlong
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_renderFrame(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.render();
    debug!("Rendered frame");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_dropWgpuState(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.drop_state();
    debug!("Dropped wgpu state");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_recreateWgpuState(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    surface: JObject,
) {
    debug!("Entering recreate_wgpu_state");
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    debug!("Recreating wgpu state");
    emulator.recreate_state(env.get_raw(), surface.as_raw());
    debug!("Recreated wgpu state");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_resizeWgpuState(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    width: jint,
    height: jint,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.resize(width as u32, height as u32);
    debug!("Resized wgpu state to {}x{}", width, height);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_onWgpuLost(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.on_lost();
    debug!("Handled wgpu lost");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_loadRom(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    rom_path: JString,
    sav_path: JString,
) -> jboolean {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    
    let rom_path_str = match env.get_string(&rom_path) {
        Ok(path) => path,
        Err(e) => {
            log::error!("Failed to get ROM path string: {}", e);
            return 0; // false
        }
    };
    
    let rom_path_buf = PathBuf::from(rom_path_str.to_str().unwrap_or(""));
    
    let sav_path_opt = if sav_path.is_null() {
        None
    } else {
        match env.get_string(&sav_path) {
            Ok(path) => {
                let path_str = path.to_str().unwrap_or("");
                if path_str.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(path_str))
                }
            }
            Err(e) => {
                log::error!("Failed to get save path string: {}", e);
                return 0; // false
            }
        }
    };
    
    match emulator.load_rom(&rom_path_buf, sav_path_opt.as_deref()) {
        Ok(()) => {
            debug!("ROM loaded successfully: {:?}", rom_path_buf);
            1 // true
        }
        Err(e) => {
            log::error!("Failed to load ROM: {}", e);
            0 // false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_pressButton(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    button_id: jint,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    
    let button = match button_id {
        0 => Button::Right,
        1 => Button::Left,
        2 => Button::Up,
        3 => Button::Down,
        4 => Button::A,
        5 => Button::B,
        6 => Button::Select,
        7 => Button::Start,
        _ => {
            log::error!("Invalid button ID: {}", button_id);
            return;
        }
    };
    
    emulator.press_button(button);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_releaseButton(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    button_id: jint,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    
    let button = match button_id {
        0 => Button::Right,
        1 => Button::Left,
        2 => Button::Up,
        3 => Button::Down,
        4 => Button::A,
        5 => Button::B,
        6 => Button::Select,
        7 => Button::Start,
        _ => {
            log::error!("Invalid button ID: {}", button_id);
            return;
        }
    };
    
    emulator.release_button(button);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_pauseEmulator(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jboolean {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    
    match emulator.pause() {
        Ok(()) => 1, // true
        Err(e) => {
            log::error!("Failed to pause emulator: {}", e);
            0 // false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_resumeEmulator(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jboolean {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    
    match emulator.resume() {
        Ok(()) => 1, // true
        Err(e) => {
            log::error!("Failed to resume emulator: {}", e);
            0 // false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_isPaused(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jboolean {
    let emulator = unsafe { &*(emulator_ptr as *const Emulator) };
    if emulator.is_paused() { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_setSpeedMultiplier(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    multiplier: jint,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.set_speed_multiplier(multiplier as u32);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_setVolume(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    volume: jfloat,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.set_volume(volume);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_toggleMute(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.toggle_mute();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_destroyEmulator(
    _env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    if emulator_ptr != 0 {
        let emulator = unsafe { Box::from_raw(emulator_ptr as *mut Emulator) };
        drop(emulator);
        debug!("Emulator destroyed");
    }
}
