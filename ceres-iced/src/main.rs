mod app;
mod gb_widget;
mod scene;

const SCREEN_MUL: u32 = 1;
const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const INIT_WIDTH: u32 = PX_WIDTH * SCREEN_MUL;
const INIT_HEIGHT: u32 = PX_HEIGHT * SCREEN_MUL;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "remind-me-later";
// const CERES_BIN: &str = "ceres";
const CERES_STYLIZED: &str = "Ceres";

#[derive(Default, Clone, Copy, PartialEq, Debug)]
pub enum Scaling {
    #[default]
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
}

impl Scaling {
    pub const ALL: [Scaling; 3] = [Scaling::Nearest, Scaling::Scale2x, Scaling::Scale3x];

    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Scaling::Nearest => Scaling::Scale2x,
            Scaling::Scale2x => Scaling::Scale3x,
            Scaling::Scale3x => Scaling::Nearest,
        }
    }
}

impl std::fmt::Display for Scaling {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scaling::Nearest => write!(f, "Nearest"),
            Scaling::Scale2x => write!(f, "Scale2x"),
            Scaling::Scale3x => write!(f, "Scale3x"),
        }
    }
}

pub fn main() -> iced::Result {
    iced::application(app::App::title, app::App::update, app::App::view)
        .subscription(app::App::subscription)
        .default_font(iced::Font {
            family: iced::font::Family::Monospace,
            ..Default::default()
        })
        .window_size(iced::Size {
            width: INIT_WIDTH as f32,
            height: INIT_HEIGHT as f32,
        })
        .resizable(true)
        .scale_factor(|_| 0.8)
        .run()
}
