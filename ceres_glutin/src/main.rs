#![feature(duration_constants)]
#![warn(
    clippy::pedantic,
    clippy::as_underscore,
    clippy::clone_on_ref_ptr,
    clippy::decimal_literal_representation,
    clippy::deref_by_slicing,
    clippy::empty_drop,
    clippy::empty_structs_with_brackets,
    clippy::float_cmp_const,
    clippy::fn_to_numeric_cast_any,
    clippy::get_unwrap,
    clippy::if_then_some_else_none,
    clippy::let_underscore_must_use,
    clippy::lossy_float_literal,
    clippy::map_err_ignore,
    clippy::mem_forget,
    clippy::mixed_read_write_in_expression,
    clippy::modulo_arithmetic,
    clippy::non_ascii_literal,
    clippy::rc_buffer,
    clippy::rc_mutex,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::same_name_method,
    clippy::self_named_module_files,
    clippy::shadow_unrelated,
    clippy::str_to_string,
    clippy::string_add,
    clippy::string_slice,
    clippy::string_to_string,
    clippy::try_err,
    clippy::unnecessary_self_imports,
    clippy::unneeded_field_pattern
)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use {
    ceres_core::Model,
    clap::{arg, builder::PossibleValuesParser},
    std::path::PathBuf,
};

mod audio;
mod emu;
mod video;

const CERES_STR: &str = "Ceres";

fn main() {
    let cli = clap::Command::new("ceres")
        .bin_name("ceres")
        .arg(arg!([rom] "rom to emulate"))
        .arg(
            arg!(-m --model <MODEL> "GB model to emulate")
                .value_parser(PossibleValuesParser::new(["dmg", "mgb", "cgb"]))
                .default_value("cgb")
                .required(false),
        )
        .get_matches();

    let model_str = cli.get_one::<String>("model").unwrap();
    let model = match model_str.as_str() {
        "dmg" => Model::Dmg,
        "mgb" => Model::Mgb,
        "cgb" => Model::Cgb,
        _ => unreachable!(),
    };

    let rom_path = Some(PathBuf::from(cli.get_one::<String>("rom").unwrap()));

    if let Some(rom_path) = rom_path {
        let emu = emu::Emu::init(model, rom_path);
        emu.run();
    }
}
