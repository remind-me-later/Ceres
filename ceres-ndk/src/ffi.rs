#![allow(non_snake_case)]

use crate::emulator::Emulator;
#[cfg(feature = "game_genie")]
use ceres_std::GameGenieCode;
use ceres_std::{Button, ColorCorrectionMode, Model, ShaderOption};
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

// Color correction mode functions
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_setColorCorrectionMode(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    mode: jint,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };

    let color_mode = match mode {
        0 => ColorCorrectionMode::CorrectCurves,
        1 => ColorCorrectionMode::Disabled,
        2 => ColorCorrectionMode::LowContrast,
        3 => ColorCorrectionMode::ModernBalanced,
        4 => ColorCorrectionMode::ModernBoostContrast,
        5 => ColorCorrectionMode::ReduceContrast,
        _ => {
            throw_illegal_argument_exception(
                &mut env,
                &format!("Invalid color correction mode: {mode}"),
            );
            return;
        }
    };

    emulator.set_color_correction_mode(color_mode);
}

// Shader option functions
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_setShaderOption(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    shader: jint,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };

    let shader_option = match shader {
        0 => ShaderOption::Crt,
        1 => ShaderOption::Lcd,
        2 => ShaderOption::Nearest,
        3 => ShaderOption::Scale2x,
        4 => ShaderOption::Scale3x,
        _ => {
            throw_illegal_argument_exception(&mut env, &format!("Invalid shader option: {shader}"));
            return;
        }
    };

    emulator.set_shader_option(shader_option);
}

// Model change functions
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_changeModel(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    model: jint,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };

    let gb_model = match model {
        0 => Model::Dmg,
        1 => Model::Cgb,
        2 => Model::Mgb,
        _ => {
            throw_illegal_argument_exception(&mut env, &format!("Invalid model: {model}"));
            return;
        }
    };

    emulator.change_model(gb_model);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_getModel(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jint {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return -1;
    }

    let emulator = unsafe { &*(emulator_ptr as *const Emulator) };

    match emulator.model() {
        Model::Dmg => 0,
        Model::Cgb => 1,
        Model::Mgb => 2,
    }
}

// Additional getter functions
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_getSpeedMultiplier(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jint {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 1;
    }

    let emulator = unsafe { &*(emulator_ptr as *const Emulator) };

    #[expect(clippy::cast_possible_wrap)]
    {
        emulator.multiplier().min(jint::MAX as u32) as jint
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_isMuted(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jboolean {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 0;
    }

    let emulator = unsafe { &*(emulator_ptr as *const Emulator) };
    u8::from(emulator.is_muted())
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_getVolume(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jfloat {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 0.0;
    }

    let emulator = unsafe { &*(emulator_ptr as *const Emulator) };
    emulator.volume()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_hasSaveData(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jboolean {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 0;
    }

    let emulator = unsafe { &*(emulator_ptr as *const Emulator) };
    u8::from(emulator.has_save_data())
}

// Game Genie functions (conditional on feature)
#[cfg(feature = "game_genie")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_activateGameGenie(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    code_str: JString,
) -> jboolean {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return 0;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };

    let code_string: String = match env.get_string(&code_str) {
        Ok(s) => s.into(),
        Err(e) => {
            throw_illegal_argument_exception(&mut env, &format!("Failed to get code string: {e}"));
            return 0;
        }
    };

    match GameGenieCode::new(&code_string) {
        Ok(code) => match emulator.activate_game_genie(code) {
            Ok(()) => 1, // true
            Err(e) => {
                throw_runtime_exception(
                    &mut env,
                    &format!("Failed to activate Game Genie code: {e}"),
                );
                0 // false
            }
        },
        Err(e) => {
            throw_illegal_argument_exception(&mut env, &format!("Invalid Game Genie code: {e}"));
            0 // false
        }
    }
}

#[cfg(feature = "game_genie")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_deactivateGameGenie(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    code_str: JString,
) {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return;
    }

    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };

    let code_string: String = match env.get_string(&code_str) {
        Ok(s) => s.into(),
        Err(e) => {
            throw_illegal_argument_exception(&mut env, &format!("Failed to get code string: {e}"));
            return;
        }
    };

    match GameGenieCode::new(&code_string) {
        Ok(code) => emulator.deactivate_game_genie(&code),
        Err(e) => {
            throw_illegal_argument_exception(&mut env, &format!("Invalid Game Genie code: {e}"));
        }
    }
}

#[cfg(feature = "game_genie")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_getActiveGameGenieCodes(
    mut env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) -> jni::sys::jobjectArray {
    if emulator_ptr == 0 {
        throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
        return std::ptr::null_mut();
    }

    let emulator = unsafe { &*(emulator_ptr as *const Emulator) };

    // Create a String array to hold the codes
    let string_class = match env.find_class("java/lang/String") {
        Ok(class) => class,
        Err(e) => {
            throw_runtime_exception(&mut env, &format!("Failed to find String class: {e}"));
            return std::ptr::null_mut();
        }
    };

    let empty_str = match env.new_string("") {
        Ok(s) => s,
        Err(e) => {
            throw_runtime_exception(&mut env, &format!("Failed to create empty string: {e}"));
            return std::ptr::null_mut();
        }
    };

    if let Some(codes) = emulator.active_game_genie_codes() {
        #[expect(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let len = codes.len().min(i32::MAX as usize) as i32;
        let array = match env.new_object_array(len, &string_class, &empty_str) {
            Ok(arr) => arr,
            Err(e) => {
                throw_runtime_exception(&mut env, &format!("Failed to create array: {e}"));
                return std::ptr::null_mut();
            }
        };

        // Fill the array with code strings
        #[expect(clippy::cast_sign_loss)]
        for (i, code) in codes.iter().enumerate().take(len as usize) {
            let code_str = match env.new_string(code.to_string()) {
                Ok(s) => s,
                Err(e) => {
                    throw_runtime_exception(
                        &mut env,
                        &format!("Failed to create code string: {e}"),
                    );
                    return std::ptr::null_mut();
                }
            };

            #[expect(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let index = i.min(i32::MAX as usize) as i32;
            if let Err(e) = env.set_object_array_element(&array, index, &code_str) {
                throw_runtime_exception(&mut env, &format!("Failed to set array element: {e}"));
                return std::ptr::null_mut();
            }
        }

        array.into_raw()
    } else {
        match env.new_object_array(0, &string_class, &empty_str) {
            Ok(arr) => arr.into_raw(),
            Err(e) => {
                throw_runtime_exception(&mut env, &format!("Failed to create empty array: {e}"));
                std::ptr::null_mut()
            }
        }
    }
}

// Screenshot function (conditional on feature)
// #[cfg(feature = "screenshot")]
// #[unsafe(no_mangle)]
// pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_saveScreenshot(
//     mut env: JNIEnv,
//     _class: JClass,
//     emulator_ptr: jlong,
//     path: JString,
// ) -> jboolean {
//     if emulator_ptr == 0 {
//         throw_illegal_argument_exception(&mut env, "Invalid emulator pointer");
//         return 0;
//     }

//     let emulator = unsafe { &*(emulator_ptr as *const Emulator) };

//     let path_str = match env.get_string(&path) {
//         Ok(jstr) => jstr.to_str().unwrap_or(""),
//         Err(e) => {
//             throw_illegal_argument_exception(&mut env, &format!("Failed to get path string: {e}"));
//             return 0;
//         }
//     };

//     if path_str.is_empty() {
//         throw_illegal_argument_exception(&mut env, "Path cannot be empty");
//         return 0;
//     }

//     match emulator.save_screenshot(path_str) {
//         Ok(()) => 1, // true
//         Err(e) => {
//             throw_io_exception(&mut env, &format!("Failed to save screenshot: {e}"));
//             0 // false
//         }
//     }
// }
