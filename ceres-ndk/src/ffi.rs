#![allow(non_snake_case)]

use crate::emulator::Emulator;
use ceres_std::Button;
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString};
use jni::sys::{jboolean, jfloat, jint, jlong};
use log::debug;
use std::path::PathBuf;

// Helper functions for throwing Java exceptions
fn throw_runtime_exception(env: &mut JNIEnv, message: &str) {
    if let Err(e) = env.throw_new("java/lang/RuntimeException", message) {
        log::error!("Failed to throw RuntimeException: {e}");
    }
}

fn throw_illegal_argument_exception(env: &mut JNIEnv, message: &str) {
    if let Err(e) = env.throw_new("java/lang/IllegalArgumentException", message) {
        log::error!("Failed to throw IllegalArgumentException: {e}");
    }
}

fn throw_io_exception(env: &mut JNIEnv, message: &str) {
    if let Err(e) = env.throw_new("java/io/IOException", message) {
        log::error!("Failed to throw IOException: {e}");
    }
}

fn throw_illegal_state_exception(env: &mut JNIEnv, message: &str) {
    if let Err(e) = env.throw_new("java/lang/IllegalStateException", message) {
        log::error!("Failed to throw IllegalStateException: {e}");
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_createEmulator(
    mut env: JNIEnv,
    _class: JClass,
) -> jlong {
    match Emulator::new() {
        Ok(emulator) => {
            let emulator = Box::new(emulator);
            let ptr = Box::into_raw(emulator);
            debug!("Emulator created at {ptr:p}");
            ptr as jlong
        }
        Err(e) => {
            throw_runtime_exception(&mut env, &format!("Failed to create emulator: {e}"));
            0 // Return null pointer on error
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_renderFrame(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    if let Err(e) = emulator.render() {
        throw_runtime_exception(&mut env, &format!("Failed to render frame: {e}"));
        return;
    }
    debug!("Rendered frame");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_dropWgpuState(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.drop_state();
    debug!("Dropped wgpu state");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_recreateWgpuState(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    surface: JObject,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    debug!("Entering recreate_wgpu_state");
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    debug!("Recreating wgpu state");
    emulator.recreate_state(env, surface);
    debug!("Recreated wgpu state");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_resizeWgpuState(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    width: jint,
    height: jint,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    #[expect(clippy::cast_sign_loss)]
    emulator.resize(width as u32, height as u32);
    debug!("Resized wgpu state to {width}x{height}");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_onWgpuLost(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

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

    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 0;
    }

    let rom_path_str = match env.get_string(&rom_path) {
        Ok(path) => path,
        Err(e) => {
            throw_illegal_argument_exception(
                &mut env,
                &format!("Failed to get ROM path string: {e}"),
            );
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
                throw_illegal_argument_exception(
                    &mut env,
                    &format!("Failed to get save path string: {e}"),
                );
                return 0; // false
            }
        }
    };

    match emulator.load_rom(&rom_path_buf, sav_path_opt.as_deref()) {
        Ok(()) => {
            debug!("ROM loaded successfully: {}", rom_path_buf.display());
            1 // true
        }
        Err(e) => {
            throw_io_exception(&mut env, &format!("Failed to load ROM: {e}"));
            0 // false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_pressButton(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    button_id: jint,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

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
            throw_illegal_argument_exception(&mut env, &format!("Invalid button ID: {button_id}"));
            return;
        }
    };

    emulator.press_button(button);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_releaseButton(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    button_id: jint,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

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
            throw_illegal_argument_exception(&mut env, &format!("Invalid button ID: {button_id}"));
            return;
        }
    };

    emulator.release_button(button);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_pauseEmulator(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jboolean {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 0;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };

    match emulator.pause() {
        Ok(()) => 1, // true
        Err(e) => {
            throw_illegal_state_exception(&mut env, &format!("Failed to pause emulator: {e}"));
            0 // false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_resumeEmulator(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jboolean {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 0;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };

    match emulator.resume() {
        Ok(()) => 1, // true
        Err(e) => {
            throw_illegal_state_exception(&mut env, &format!("Failed to resume emulator: {e}"));
            0 // false
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_isPaused(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jboolean {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 0;
    }

    let emulator = unsafe { &*(emulator_ptr as *const Emulator) };
    u8::from(emulator.is_paused())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_setSpeedMultiplier(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    multiplier: jint,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    #[expect(clippy::cast_sign_loss)]
    emulator.set_speed_multiplier(multiplier as u32);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_setVolume(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    volume: jfloat,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.set_volume(volume);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_toggleMute(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

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

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_saveRam(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    path: JString,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    let path = match env.get_string(&path) {
        Ok(jstr) => {
            let jstr_path: String = jstr.into();
            if jstr_path.is_empty() {
                throw_illegal_argument_exception(&mut env, "Path cannot be empty");
                return;
            }
            if let Err(e) = emulator.save_data(&jstr_path) {
                throw_io_exception(&mut env, &format!("Failed to save RAM: {e}"));
            }
            jstr_path
        }
        Err(e) => {
            throw_illegal_argument_exception(&mut env, &format!("Couldn't get string: {e}"));
            return;
        }
    };

    if let Err(e) = emulator.save_data(&path) {
        throw_io_exception(&mut env, &format!("Failed to save RAM: {e}"));
    }
}
