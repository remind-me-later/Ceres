mod audio;
mod gb_widget;
mod scene;

use std::time::Instant;

use iced::widget::{button, column, container, pick_list, row, shader, text};
use iced::{
    window, Alignment, Application, Command, Element, Length, Settings, Subscription, Theme,
};

const SCREEN_MUL: u32 = 3;
const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const INIT_WIDTH: u32 = PX_WIDTH * SCREEN_MUL;
const INIT_HEIGHT: u32 = PX_HEIGHT * SCREEN_MUL;

const QUALIFIER: &str = "com";
const ORGANIZATION: &str = "remind-me-later";
const CERES_BIN: &str = "ceres";
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
    App::run(Settings {
        window: iced::window::Settings {
            size: iced::Size {
                width: INIT_WIDTH as f32,
                height: INIT_HEIGHT as f32,
            },
            resizable: true,
            visible: true,
            ..iced::window::Settings::default()
        },
        default_font: iced::Font {
            family: iced::font::Family::Monospace,
            ..Default::default()
        },
        ..Settings::default()
    })
}

struct App {
    widget: gb_widget::GbWidget,
    _audio: audio::State,

    project_dirs: directories::ProjectDirs,
}

impl Default for App {
    fn default() -> Self {
        let project_dirs =
            directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, CERES_STYLIZED).unwrap();
        let audio = audio::State::new();

        App {
            widget: gb_widget::GbWidget::new(ceres_core::Model::Cgb, &project_dirs, None, &audio),
            _audio: audio,
            project_dirs,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    ScalingChanged(Scaling),
    OpenButtonPressed,
    ExportButtonPressed,
    Tick(Instant),
}

impl Application for App {
    type Message = Message;
    type Executor = iced::executor::Default;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (App::default(), Command::none())
    }

    fn title(&self) -> String {
        String::from("Ceres")
    }

    fn update(&mut self, message: Message) -> Command<Self::Message> {
        match message {
            Message::ScalingChanged(scaling) => {
                self.widget.set_scaling(scaling);
            }
            Message::OpenButtonPressed => {}
            Message::ExportButtonPressed => {}
            Message::Tick(_) => {}
        }

        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let content = {
            let top_row = row![
                text("Scaling mode"),
                pick_list(
                    Scaling::ALL,
                    Some(self.widget.scaling()),
                    Message::ScalingChanged
                )
                .width(Length::Shrink)
            ];

            let shader = shader(self.widget.scene())
                .width(Length::Fill)
                .height(Length::Fill);

            column![top_row, shader].spacing(20)
        };

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::GruvboxLight
    }

    fn subscription(&self) -> Subscription<Message> {
        window::frames().map(Message::Tick)
    }
}
