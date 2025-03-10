#![expect(unexpected_cfgs)]

use crate::{CeresEvent, ScalingOption, ShaderOption};
use cocoa::{
    appkit::{NSApp, NSApplication, NSMenu, NSMenuItem, NSOpenPanel, NSSavePanel},
    base::{id, nil},
    foundation::{NSAutoreleasePool, NSString, NSURL},
};
use objc::{
    class, msg_send,
    runtime::{NO, YES},
    sel, sel_impl,
};
use std::sync::OnceLock;
use winit::event_loop::EventLoopProxy;

// Store event proxy to send events back to the main thread
static EVENT_PROXY: OnceLock<EventLoopProxy<CeresEvent>> = OnceLock::new();

pub fn set_event_proxy(proxy: EventLoopProxy<CeresEvent>) {
    if EVENT_PROXY.set(proxy).is_err() {
        eprintln!("Event proxy already set");
    }
}

// Callback handlers for shader menu items
extern "C" fn change_shader_nearest() {
    send_shader_event(ShaderOption::Nearest);
}

extern "C" fn change_shader_scale2x() {
    send_shader_event(ShaderOption::Scale2x);
}

extern "C" fn change_shader_scale3x() {
    send_shader_event(ShaderOption::Scale3x);
}

extern "C" fn change_shader_lcd() {
    send_shader_event(ShaderOption::Lcd);
}

extern "C" fn change_shader_crt() {
    send_shader_event(ShaderOption::Crt);
}

// Callback handlers for scaling menu items
extern "C" fn change_scaling_pixel_perfect() {
    send_scaling_event(ScalingOption::PixelPerfect);
}

extern "C" fn change_scaling_fit_window() {
    send_scaling_event(ScalingOption::FitWindow);
}

// Callback handlers for speed menu items
extern "C" fn change_speed_1x() {
    send_speed_event(1);
}

extern "C" fn change_speed_2x() {
    send_speed_event(2);
}

extern "C" fn change_speed_4x() {
    send_speed_event(4);
}

// Callback handler for open file menu item
extern "C" fn open_rom_file() {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);

        let open_panel = NSOpenPanel::openPanel(nil);
        open_panel.setCanChooseFiles_(YES);
        open_panel.setCanChooseDirectories_(NO);
        open_panel.setAllowsMultipleSelection_(NO);

        // Set file types (GB and GBC ROMs)
        let allowed_file_types = cocoa::foundation::NSArray::arrayWithObjects(
            nil,
            &[
                NSString::alloc(nil).init_str("gb").autorelease(),
                NSString::alloc(nil).init_str("gbc").autorelease(),
                NSString::alloc(nil).init_str("rom").autorelease(),
            ],
        );
        let _: () = msg_send![open_panel, setAllowedFileTypes:allowed_file_types];

        // Show the panel
        let result = open_panel.runModal();

        if result == cocoa::appkit::NSModalResponse::NSModalResponseOk {
            let path_str = NSString::UTF8String(NSURL::path(open_panel.URL()));
            let path = std::path::PathBuf::from(
                std::ffi::CStr::from_ptr(path_str)
                    .to_string_lossy()
                    .into_owned(),
            );

            if let Some(proxy) = EVENT_PROXY.get() {
                proxy.send_event(CeresEvent::OpenRomFile(path)).ok();
            }
        }

        pool.drain();
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
        let pool = NSAutoreleasePool::new(nil);

        let app = NSApplication::sharedApplication(nil);

        let menu_bar = NSMenu::new(nil).autorelease();
        let app_menu_item = NSMenuItem::new(nil).autorelease();

        app.setMainMenu_(menu_bar);
        menu_bar.addItem_(app_menu_item);

        let app_menu = NSMenu::new(nil).autorelease();
        let quit_title = NSString::alloc(nil).init_str("Quit");

        let about_title =
            NSString::alloc(nil).init_str(&format!("About {}", crate::CERES_STYLIZED));
        let about_action = sel!(orderFrontStandardAboutPanel:);
        let about_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                about_title,
                about_action,
                NSString::alloc(nil).init_str(""),
            )
            .autorelease();
        app_menu.addItem_(about_item);

        let quit_action = sel!(terminate:);
        let quit_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                quit_title,
                quit_action,
                NSString::alloc(nil).init_str("q"),
            )
            .autorelease();
        app_menu.addItem_(quit_item);

        app_menu_item.setSubmenu_(app_menu);

        let file_menu_item = NSMenuItem::new(nil).autorelease();
        let file_menu = NSMenu::new(nil).autorelease();
        let file_title = NSString::alloc(nil).init_str("File");

        cocoa::appkit::NSButton::setTitle_(file_menu_item, file_title);
        menu_bar.addItem_(file_menu_item);

        let cls = class!(NSObject);
        let sel_open_file = sel!(openRomFile:);

        let types = format!(
            "{}{}",
            objc_encode::Encoding::Void,
            objc_encode::Encoding::Object,
        );

        // Add the open file method
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_open_file,
            open_rom_file,
            types.as_ptr().cast(),
        );

        // "Open ROM..." menu item
        let open_title = NSString::alloc(nil).init_str("Open...");
        let open_item = NSMenuItem::alloc(nil)
            .initWithTitle_action_keyEquivalent_(
                open_title,
                sel_open_file,
                NSString::alloc(nil).init_str("o"), // Command+O shortcut
            )
            .autorelease();
        open_item.setTarget_(NSApp());
        file_menu.addItem_(open_item);

        // Set the file menu
        file_menu_item.setSubmenu_(file_menu);

        // Add a View menu with shader and scaling options
        let view_menu_item = NSMenuItem::new(nil).autorelease();
        let view_menu = NSMenu::new(nil).autorelease();
        let view_title = NSString::alloc(nil).init_str("View");

        cocoa::appkit::NSButton::setTitle_(view_menu_item, view_title);
        menu_bar.addItem_(view_menu_item);

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
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_nearest,
            change_shader_nearest,
            types.as_ptr().cast(),
        );
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_scale2x,
            change_shader_scale2x,
            types.as_ptr().cast(),
        );
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_scale3x,
            change_shader_scale3x,
            types.as_ptr().cast(),
        );
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_lcd,
            change_shader_lcd,
            types.as_ptr().cast(),
        );
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_crt,
            change_shader_crt,
            types.as_ptr().cast(),
        );

        // Register scaling methods
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_pixel_perfect,
            change_scaling_pixel_perfect,
            types.as_ptr().cast(),
        );
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_fit_window,
            change_scaling_fit_window,
            types.as_ptr().cast(),
        );

        // Register speed methods
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_speed_1x,
            change_speed_1x,
            types.as_ptr().cast(),
        );
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_speed_2x,
            change_speed_2x,
            types.as_ptr().cast(),
        );
        let _ = objc::runtime::class_addMethod(
            std::ptr::from_ref(cls).cast_mut(),
            sel_speed_4x,
            change_speed_4x,
            types.as_ptr().cast(),
        );

        // Create shader submenu
        let shader_submenu_item = NSMenuItem::new(nil).autorelease();
        let shader_submenu = NSMenu::new(nil).autorelease();
        let shader_title = NSString::alloc(nil).init_str("Shader");
        cocoa::appkit::NSButton::setTitle_(shader_submenu_item, shader_title);
        view_menu.addItem_(shader_submenu_item);

        add_menu_item(shader_submenu, "Nearest", sel_nearest, "1");
        add_menu_item(shader_submenu, "Scale2x", sel_scale2x, "2");
        add_menu_item(shader_submenu, "Scale3x", sel_scale3x, "3");
        add_menu_item(shader_submenu, "LCD Effect", sel_lcd, "4");
        add_menu_item(shader_submenu, "CRT Effect", sel_crt, "5");

        shader_submenu_item.setSubmenu_(shader_submenu);

        let scaling_submenu_item = NSMenuItem::new(nil).autorelease();
        let scaling_submenu = NSMenu::new(nil).autorelease();
        let scaling_title = NSString::alloc(nil).init_str("Scaling");
        cocoa::appkit::NSButton::setTitle_(scaling_submenu_item, scaling_title);
        view_menu.addItem_(scaling_submenu_item);

        add_menu_item(scaling_submenu, "Pixel Perfect", sel_pixel_perfect, "p");
        add_menu_item(scaling_submenu, "Fit Window", sel_fit_window, "w");

        scaling_submenu_item.setSubmenu_(scaling_submenu);

        view_menu.addItem_(NSMenuItem::separatorItem(nil));

        let speed_submenu_item = NSMenuItem::new(nil).autorelease();
        let speed_submenu = NSMenu::new(nil).autorelease();
        let speed_title = NSString::alloc(nil).init_str("Speed");
        cocoa::appkit::NSButton::setTitle_(speed_submenu_item, speed_title);
        view_menu.addItem_(speed_submenu_item);

        add_menu_item(speed_submenu, "Normal Speed (1x)", sel_speed_1x, "1");
        add_menu_item(speed_submenu, "Double Speed (2x)", sel_speed_2x, "2");
        add_menu_item(speed_submenu, "Quadruple Speed (4x)", sel_speed_4x, "4");

        speed_submenu_item.setSubmenu_(speed_submenu);

        view_menu_item.setSubmenu_(view_menu);

        pool.drain();
    }
}

unsafe fn add_menu_item(menu: id, title: &str, action: objc::runtime::Sel, key_equivalent: &str) {
    let title_str = NSString::alloc(nil).init_str(title);
    let key_str = NSString::alloc(nil).init_str(key_equivalent);

    let item = NSMenuItem::alloc(nil)
        .initWithTitle_action_keyEquivalent_(title_str, action, key_str)
        .autorelease();

    item.setTarget_(NSApp());

    menu.addItem_(item);
}
