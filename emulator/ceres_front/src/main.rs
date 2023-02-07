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
    clippy::cast_possible_wrap,
    clippy::similar_names
)]

use {
    ceres_core::Model,
    clap::{builder::PossibleValuesParser, Arg, Command},
    const_format::formatcp,
    std::path::PathBuf,
};

mod audio;
mod main_loop;
mod video;

const CERES_BIN: &str = "ceres";
const CERES_STYLIZED: &str = "Ceres";
const GB_STYLIZED: &str = "Game Boy";
const GBC_STYLIZED: &str = formatcp!("{GB_STYLIZED}/Color");
const ABOUT: &str = formatcp!("A (very experimental) {GBC_STYLIZED} emulator.");
const AFTER_HELP: &str = formatcp!(
    "KEY BINDINGS:

    | Gameboy | Emulator  |
    | ------- | --------- |
    | Dpad    | WASD      |
    | A       | K         |
    | B       | L         |
    | Start   | Return    |
    | Select  | Backspace |"
);

fn main() {
    let args = Command::new(CERES_BIN)
        .bin_name(CERES_BIN)
        .about(ABOUT)
        .after_help(AFTER_HELP)
        .arg(
            Arg::new("file")
                .required(true)
                .help("{GBC_STYLIZED} ROM file to emulate.")
                .long_help(formatcp!(
                    "{GBC_STYLIZED} ROM file to emulate. Extension doesn't matter, the emulator \
                     will check the file is a valid {GB_STYLIZED} ROM reading it's header. \
                     Doesn't accept compressed (zip) files."
                )),
        )
        .arg(
            Arg::new("model")
                .short('m')
                .long("model")
                .help(formatcp!("{GB_STYLIZED} model to emulate"))
                .value_parser(PossibleValuesParser::new(["dmg", "mgb", "cgb"]))
                .default_value("cgb")
                .required(false),
        )
        .get_matches();

    let model_str = args.get_one::<String>("model").unwrap();
    let model = match model_str.as_str() {
        "dmg" => Model::Dmg,
        "mgb" => Model::Mgb,
        "cgb" => Model::Cgb,
        _ => unreachable!(),
    };

    let path = PathBuf::from(args.get_one::<String>("file").unwrap());

    let emu = main_loop::Emu::new(model, path);
    emu.run();
}
