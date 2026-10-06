//! GL function lookup through libepoxy, the library GTK uses to manage GL
//! contexts.

use core::ffi::c_void;
use std::sync::OnceLock;

fn library() -> &'static libloading::Library {
    static LIBRARY: OnceLock<libloading::Library> = OnceLock::new();

    LIBRARY.get_or_init(|| {
        // SAFETY: libepoxy has no initialization routines with preconditions.
        unsafe { libloading::Library::new("libepoxy.so.0") }.expect("couldn't load libepoxy")
    })
}

/// The address of the GL function `name`, or null if libepoxy doesn't have it.
///
/// libepoxy exports `epoxy_<name>` as a variable holding a pointer to the
/// function (a dispatch table entry), not as the function itself, so the
/// symbol has to be read once to get the dispatcher of the currently bound GL
/// context.
pub fn get_proc_address(name: &str) -> *const c_void {
    let symbol = format!("epoxy_{name}");

    // SAFETY: the symbol is the address of a pointer variable in libepoxy,
    // which stays loaded for the rest of the program.
    unsafe {
        library()
            .get::<*const *const c_void>(symbol.as_bytes())
            .map_or(core::ptr::null(), |entry| **entry)
    }
}
