use colorful::{Color, Colorful};

#[derive(Debug)]
pub struct Error {
    msg: String,
}

impl Error {
    pub fn new<E: std::fmt::Display>(error: E) -> Self {
        Self {
            msg: error.to_string(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for Error {}

pub fn print<E: std::fmt::Display>(error: E) -> ! {
    println!(
        "{}{} {} {}",
        super::CERES_STR.bold(),
        ":".bold(),
        "error:".color(Color::Red).bold(),
        error
    );
    std::process::exit(1);
}
