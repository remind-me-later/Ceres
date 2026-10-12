//! Translates the screen shader, which the wgpu frontends use as WGSL, to the
//! GLSL this frontend's OpenGL renderer compiles (`src/gl_area/renderer.rs`).
//!
//! Every resource gets an explicit binding (`BINDINGS`), so the renderer binds
//! by number and does not depend on the names naga generates.

use naga::{
    ResourceBinding,
    back::glsl::{BindingMap, Options, PipelineOptions, Version, Writer, WriterFlags},
    proc::BoundsCheckPolicies,
    valid::{Capabilities, ValidationFlags, Validator},
};
use std::{env, fs, path::PathBuf};

const WGSL: &str = "../ceres-std/shader/gb_screen.wgsl";

/// The WGSL `(group, binding)` of each resource and its GLSL binding, which
/// `src/gl_area/renderer.rs` uses: texture units for the frames, uniform
/// buffer binding points for the uniforms.
const BINDINGS: [((u32, u32), u8); 4] = [
    // The current frame and the previous one.
    ((0, 0), 0),
    ((0, 2), 2),
    // The image's size in the viewport, and the shader option.
    ((1, 0), 0),
    ((1, 1), 1),
];

fn main() {
    println!("cargo:rerun-if-changed={WGSL}");
    println!("cargo:rerun-if-changed=build.rs");

    let source = fs::read_to_string(WGSL).expect("the WGSL shader");
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string_with_path(&source, WGSL)));
    let info = Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string_with_path(&source, WGSL)));

    let binding_map: BindingMap = BINDINGS
        .into_iter()
        .map(|((group, binding), glsl)| (ResourceBinding { group, binding }, glsl))
        .collect();
    let options = Options {
        // GLSL ES 3.10, as the renderer's GL context accepts.
        version: Version::new_gles(310),
        // GL's own coordinate space: the renderer draws into GTK's
        // framebuffer as is (no Y flip).
        writer_flags: WriterFlags::empty(),
        binding_map,
        ..Options::default()
    };
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    for (stage, entry_point, file) in [
        (naga::ShaderStage::Vertex, "vs_main", "vshader.vert"),
        (naga::ShaderStage::Fragment, "fs_main", "fshader.frag"),
    ] {
        let pipeline_options = PipelineOptions {
            shader_stage: stage,
            entry_point: entry_point.to_owned(),
            multiview: None,
        };
        let mut glsl = String::new();
        Writer::new(
            &mut glsl,
            &module,
            &info,
            &options,
            &pipeline_options,
            BoundsCheckPolicies::default(),
        )
        .and_then(|mut writer| writer.write())
        .unwrap_or_else(|e| panic!("translating {entry_point} to GLSL: {e}"));
        fs::write(out_dir.join(file), glsl).expect("writing the GLSL");
    }
}
