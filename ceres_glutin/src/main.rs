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
    clap::{ArgEnum, Parser},
    std::path::PathBuf,
};

mod audio;
mod emu;
mod video;

const CERES_STR: &str = "Ceres";

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    rom_path: String,

    #[clap(short = 'm', long = "model", arg_enum)]
    model: Option<CliModel>,
}

#[derive(Clone, ArgEnum)]
enum CliModel {
    Dmg,
    Mgb,
    Cgb,
}

fn main() {
    let cli = Cli::parse();

    let model = cli.model.map_or(Model::Cgb, move |s| match s {
        CliModel::Dmg => Model::Dmg,
        CliModel::Mgb => Model::Mgb,
        CliModel::Cgb => Model::Cgb,
    });

    let rom_path = Some(PathBuf::from(cli.rom_path));

    if let Some(rom_path) = rom_path {
        let emu = emu::Emu::init(model, rom_path);
        emu.run();
    }
}
