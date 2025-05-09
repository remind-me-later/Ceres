use crate::{AppOption, CeresEvent, ScalingOption, ShaderOption};
use objc2::{
    Encoding, class,
    ffi::class_addMethod,
    rc::Retained,
    runtime::{AnyObject, Sel},
    sel,
};
use objc2_app_kit::{NSApplication, NSColor, NSMenu, NSMenuItem, NSView};
use objc2_foundation::{MainThreadMarker, NSString, ns_string};
use std::{ptr, sync::OnceLock};
use winit::event_loop::EventLoopProxy;

static EVENT_PROXY: OnceLock<EventLoopProxy<CeresEvent>> = OnceLock::new();

pub fn set_event_proxy(proxy: EventLoopProxy<CeresEvent>) {
    if EVENT_PROXY.set(proxy).is_err() {
        eprintln!("Event proxy already set");
    }
}

extern "C-unwind" fn change_shader_nearest() {
    send_shader_event(ShaderOption::Nearest);
}

extern "C-unwind" fn change_shader_scale2x() {
    send_shader_event(ShaderOption::Scale2x);
}

extern "C-unwind" fn change_shader_scale3x() {
    send_shader_event(ShaderOption::Scale3x);
}

extern "C-unwind" fn change_shader_lcd() {
    send_shader_event(ShaderOption::Lcd);
}

extern "C-unwind" fn change_shader_crt() {
    send_shader_event(ShaderOption::Crt);
}

extern "C-unwind" fn change_scaling_pixel_perfect() {
    send_scaling_event(ScalingOption::PixelPerfect);
}

extern "C-unwind" fn change_scaling_fit_window() {
    send_scaling_event(ScalingOption::Stretch);
}

extern "C-unwind" fn change_speed_1x() {
    send_speed_event(1);
}

extern "C-unwind" fn change_speed_2x() {
    send_speed_event(2);
}

extern "C-unwind" fn change_speed_4x() {
    send_speed_event(4);
}

extern "C-unwind" fn toggle_pause() {
    if let Some(proxy) = EVENT_PROXY.get() {
        proxy.send_event(CeresEvent::TogglePause).ok();
    }
}

extern "C-unwind" fn open_rom_file() {
    if let Some(proxy) = EVENT_PROXY.get() {
        proxy.send_event(CeresEvent::OpenRomFile).ok();
    }
}

extern "C-unwind" fn change_model_dmg() {
    send_model_event(crate::Model::Dmg);
}

extern "C-unwind" fn change_model_mgb() {
    send_model_event(crate::Model::Mgb);
}

extern "C-unwind" fn change_model_cgb() {
    send_model_event(crate::Model::Cgb);
}

fn send_shader_event(shader: ShaderOption) {
    if let Some(proxy) = EVENT_PROXY.get() {
        proxy.send_event(CeresEvent::ChangeShader(shader)).ok();
    }
}

fn send_scaling_event(scaling: ScalingOption) {
    if let Some(proxy) = EVENT_PROXY.get() {
        proxy.send_event(CeresEvent::ChangeScaling(scaling)).ok();
    }
}

fn send_speed_event(speed: u32) {
    if let Some(proxy) = EVENT_PROXY.get() {
        proxy.send_event(CeresEvent::ChangeSpeed(speed)).ok();
    }
}

fn send_model_event(model: crate::Model) {
    if let Some(proxy) = EVENT_PROXY.get() {
        proxy.send_event(CeresEvent::ChangeModel(model)).ok();
    }
}

#[expect(clippy::too_many_lines)]
pub fn create_menu_bar() {
    unsafe {
        #[expect(clippy::unwrap_used)]
        let mtm = MainThreadMarker::new().unwrap();
        let app = NSApplication::sharedApplication(mtm);

        let menu_bar = NSMenu::new(mtm);
        let app_menu_item = NSMenuItem::new(mtm);

        app.setMainMenu(Some(&menu_bar));
        menu_bar.addItem(&app_menu_item);

        let app_menu = NSMenu::new(mtm);
        let quit_title = ns_string!("Quit");

        let about_title = ns_string!("About");
        let about_action = sel!(orderFrontStandardAboutPanel:);
        let about_item = NSMenuItem::new(mtm);
        about_item.setTitle(about_title);
        about_item.setAction(Some(about_action));

        app_menu.addItem(&about_item);

        let quit_action = sel!(terminate:);
        let quit_item = NSMenuItem::new(mtm);
        quit_item.setTitle(quit_title);
        quit_item.setAction(Some(quit_action));
        quit_item.setKeyEquivalent(ns_string!("q"));
        app_menu.addItem(&quit_item);

        app_menu_item.setSubmenu(Some(&app_menu));

        let file_menu_item = NSMenuItem::new(mtm);
        let file_menu = NSMenu::new(mtm);
        let file_title = ns_string!("File");

        file_menu_item.setTitle(file_title);
        menu_bar.addItem(&file_menu_item);

        let cls = class!(NSObject);
        let sel_open_file = sel!(openRomFile:);

        let types = format!("{}{}", Encoding::Void, Encoding::Object);

        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_open_file,
            open_rom_file,
            types.as_ptr().cast(),
        );

        add_menu_item(&app, mtm, &file_menu, "Open...", sel_open_file, Some("o"));

        file_menu_item.setSubmenu(Some(&file_menu));

        let view_menu_item = NSMenuItem::new(mtm);
        let view_menu = NSMenu::new(mtm);
        let view_title = ns_string!("View");

        view_menu_item.setTitle(view_title);
        menu_bar.addItem(&view_menu_item);

        let sel_nearest = sel!(changeShaderNearest:);
        let sel_scale2x = sel!(changeShaderScale2x:);
        let sel_scale3x = sel!(changeShaderScale3x:);
        let sel_lcd = sel!(changeShaderLcd:);
        let sel_crt = sel!(changeShaderCrt:);
        let sel_pixel_perfect = sel!(changeScalingPixelPerfect:);
        let sel_fit_window = sel!(changeScalingFitWindow:);
        let sel_speed_1x = sel!(changeSpeed1x:);
        let sel_speed_2x = sel!(changeSpeed2x:);
        let sel_speed_4x = sel!(changeSpeed4x:);
        let sel_toggle_pause = sel!(togglePause:);
        let sel_model_dmg = sel!(changeModelDmg:);
        let sel_model_mgb = sel!(changeModelMgb:);
        let sel_model_cgb = sel!(changeModelCgb:);

        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_nearest,
            change_shader_nearest,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_scale2x,
            change_shader_scale2x,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_scale3x,
            change_shader_scale3x,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_lcd,
            change_shader_lcd,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_crt,
            change_shader_crt,
            types.as_ptr().cast(),
        );

        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_pixel_perfect,
            change_scaling_pixel_perfect,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_fit_window,
            change_scaling_fit_window,
            types.as_ptr().cast(),
        );

        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_speed_1x,
            change_speed_1x,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_speed_2x,
            change_speed_2x,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_speed_4x,
            change_speed_4x,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_toggle_pause,
            toggle_pause,
            types.as_ptr().cast(),
        );

        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_model_dmg,
            change_model_dmg,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_model_mgb,
            change_model_mgb,
            types.as_ptr().cast(),
        );
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_model_cgb,
            change_model_cgb,
            types.as_ptr().cast(),
        );

        let shader_submenu_item = NSMenuItem::new(mtm);
        let shader_submenu = NSMenu::new(mtm);
        let shader_title = ns_string!("Shader");
        shader_submenu_item.setTitle(shader_title);
        view_menu.addItem(&shader_submenu_item);

        for shader in ShaderOption::iter() {
            let action = match shader {
                ShaderOption::Nearest => sel_nearest,
                ShaderOption::Scale2x => sel_scale2x,
                ShaderOption::Scale3x => sel_scale3x,
                ShaderOption::Lcd => sel_lcd,
                ShaderOption::Crt => sel_crt,
            };
            add_menu_item(&app, mtm, &shader_submenu, shader.str(), action, None);
        }

        shader_submenu_item.setSubmenu(Some(&shader_submenu));

        let scaling_submenu_item = NSMenuItem::new(mtm);
        let scaling_submenu = NSMenu::new(mtm);
        let scaling_title = ns_string!("Scaling");
        scaling_submenu_item.setTitle(scaling_title);
        view_menu.addItem(&scaling_submenu_item);

        for scaling in ScalingOption::iter() {
            let action = match scaling {
                ScalingOption::PixelPerfect => sel_pixel_perfect,
                ScalingOption::Stretch => sel_fit_window,
            };
            add_menu_item(&app, mtm, &scaling_submenu, scaling.str(), action, None);
        }

        scaling_submenu_item.setSubmenu(Some(&scaling_submenu));

        view_menu.addItem(&NSMenuItem::separatorItem(mtm));

        let speed_submenu_item = NSMenuItem::new(mtm);
        let speed_submenu = NSMenu::new(mtm);
        let speed_title = ns_string!("Speed");
        speed_submenu_item.setTitle(speed_title);
        view_menu.addItem(&speed_submenu_item);

        add_menu_item(
            &app,
            mtm,
            &speed_submenu,
            "Pause",
            sel_toggle_pause,
            Some("p"),
        );
        add_menu_item(
            &app,
            mtm,
            &speed_submenu,
            "Normal Speed (1x)",
            sel_speed_1x,
            None,
        );
        add_menu_item(
            &app,
            mtm,
            &speed_submenu,
            "Double Speed (2x)",
            sel_speed_2x,
            None,
        );
        add_menu_item(
            &app,
            mtm,
            &speed_submenu,
            "Quadruple Speed (4x)",
            sel_speed_4x,
            None,
        );

        speed_submenu_item.setSubmenu(Some(&speed_submenu));

        view_menu_item.setSubmenu(Some(&view_menu));

        // Create Model menu
        let model_menu_item = NSMenuItem::new(mtm);
        let model_menu = NSMenu::new(mtm);
        let model_title = ns_string!("Model");
        model_menu_item.setTitle(model_title);
        menu_bar.addItem(&model_menu_item);

        // Add model options to the model menu
        add_menu_item(
            &app,
            mtm,
            &model_menu,
            "Game Boy (DMG)",
            sel_model_dmg,
            None,
        );
        add_menu_item(
            &app,
            mtm,
            &model_menu,
            "Game Boy Pocket (MGB)",
            sel_model_mgb,
            None,
        );
        add_menu_item(
            &app,
            mtm,
            &model_menu,
            "Game Boy Color (CGB)",
            sel_model_cgb,
            None,
        );

        model_menu_item.setSubmenu(Some(&model_menu));
    }
}

unsafe fn add_menu_item(
    app: &NSApplication,
    mtm: MainThreadMarker,
    menu: &NSMenu,
    title: &str,
    action: Sel,
    key_equivalent: Option<&str>,
) {
    unsafe {
        let title_str = NSString::from_str(title);

        let item = NSMenuItem::new(mtm);
        item.setTitle(&title_str);
        item.setAction(Some(action));

        if let Some(key_equivalent) = key_equivalent {
            let key_str = NSString::from_str(key_equivalent);
            item.setKeyEquivalent(&key_str);
        }

        item.setTarget(Some(app));

        menu.addItem(&item);
    }
}

pub fn setup_ns_view(ns_view: *mut AnyObject) {
    unsafe {
        if let Some(ns_view) =
            Retained::retain(ns_view).and_then(|ns_view| ns_view.downcast::<NSView>().ok())
        {
            if let Some(w) = ns_view.window() {
                w.setBackgroundColor(Some(&NSColor::purpleColor()));
            }
        }
    }
}
