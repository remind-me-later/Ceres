use crate::{CeresEvent, ScalingOption, ShaderOption};
use objc2::{class, ffi::class_addMethod, runtime::Sel, sel, Encoding};
use objc2_app_kit::{NSApplication, NSMenu, NSMenuItem, NSModalResponseOK, NSOpenPanel};
use objc2_foundation::{ns_string, MainThreadMarker, NSArray, NSString, NSURL};
use std::{ptr, sync::OnceLock};
use winit::event_loop::EventLoopProxy;

// Store event proxy to send events back to the main thread
static EVENT_PROXY: OnceLock<EventLoopProxy<CeresEvent>> = OnceLock::new();

pub fn set_event_proxy(proxy: EventLoopProxy<CeresEvent>) {
    if EVENT_PROXY.set(proxy).is_err() {
        eprintln!("Event proxy already set");
    }
}

// Callback handlers for shader menu items
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

// Callback handlers for scaling menu items
extern "C-unwind" fn change_scaling_pixel_perfect() {
    send_scaling_event(ScalingOption::PixelPerfect);
}

extern "C-unwind" fn change_scaling_fit_window() {
    send_scaling_event(ScalingOption::FitWindow);
}

// Callback handlers for speed menu items
extern "C-unwind" fn change_speed_1x() {
    send_speed_event(1);
}

extern "C-unwind" fn change_speed_2x() {
    send_speed_event(2);
}

extern "C-unwind" fn change_speed_4x() {
    send_speed_event(4);
}

// Callback handler for open file menu item
extern "C-unwind" fn open_rom_file() {
    unsafe {
        #[expect(clippy::unwrap_used)]
        let mtm: MainThreadMarker = MainThreadMarker::new().unwrap();

        let open_panel = NSOpenPanel::openPanel(mtm);
        open_panel.setCanChooseFiles(true);
        open_panel.setCanChooseDirectories(false);
        open_panel.setAllowsMultipleSelection(false);

        #[expect(deprecated)]
        open_panel.setAllowedFileTypes(Some(&NSArray::from_slice(&[
            ns_string!("gb"),
            ns_string!("gbc"),
            ns_string!("rom"),
        ])));

        // Show the panel
        let result = open_panel.runModal();

        if result == NSModalResponseOK {
            #[expect(clippy::unwrap_used)]
            let path_str = NSString::UTF8String(&NSURL::path(&open_panel.URL().unwrap()).unwrap());
            let path = std::path::PathBuf::from(
                std::ffi::CStr::from_ptr(path_str)
                    .to_string_lossy()
                    .into_owned(),
            );

            if let Some(proxy) = EVENT_PROXY.get() {
                proxy.send_event(CeresEvent::OpenRomFile(path)).ok();
            }
        }
    }
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
        // about_item.setKeyEquivalent(ns_string!(""));

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

        // Add the open file method
        let _ = class_addMethod(
            ptr::from_ref(cls).cast_mut(),
            sel_open_file,
            open_rom_file,
            types.as_ptr().cast(),
        );

        // "Open ROM..." menu item
        add_menu_item(&app, mtm, &file_menu, "Open...", sel_open_file, Some("o"));

        // Set the file menu
        file_menu_item.setSubmenu(Some(&file_menu));

        // Add a View menu with shader and scaling options
        let view_menu_item = NSMenuItem::new(mtm);
        let view_menu = NSMenu::new(mtm);
        let view_title = ns_string!("View");

        view_menu_item.setTitle(view_title);
        menu_bar.addItem(&view_menu_item);

        // Register selectors
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

        // Register shader methods
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

        // Register scaling methods
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

        // Register speed methods
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

        // Create shader submenu
        let shader_submenu_item = NSMenuItem::new(mtm);
        let shader_submenu = NSMenu::new(mtm);
        let shader_title = ns_string!("Shader");
        // cocoa::appkit::NSButton::setTitle_(shader_submenu_item, shader_title);
        shader_submenu_item.setTitle(shader_title);
        view_menu.addItem(&shader_submenu_item);

        add_menu_item(&app, mtm, &shader_submenu, "Nearest", sel_nearest, None);
        add_menu_item(&app, mtm, &shader_submenu, "Scale2x", sel_scale2x, None);
        add_menu_item(&app, mtm, &shader_submenu, "Scale3x", sel_scale3x, None);
        add_menu_item(&app, mtm, &shader_submenu, "LCD Effect", sel_lcd, None);
        add_menu_item(&app, mtm, &shader_submenu, "CRT Effect", sel_crt, None);

        shader_submenu_item.setSubmenu(Some(&shader_submenu));

        let scaling_submenu_item = NSMenuItem::new(mtm);
        let scaling_submenu = NSMenu::new(mtm);
        let scaling_title = ns_string!("Scaling");
        scaling_submenu_item.setTitle(scaling_title);
        view_menu.addItem(&scaling_submenu_item);

        add_menu_item(
            &app,
            mtm,
            &scaling_submenu,
            "Pixel Perfect",
            sel_pixel_perfect,
            None,
        );
        add_menu_item(
            &app,
            mtm,
            &scaling_submenu,
            "Fit Window",
            sel_fit_window,
            None,
        );

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
