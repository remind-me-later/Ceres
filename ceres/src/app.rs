use crate::{audio, gb_widget, Scaling, CERES_STYLIZED, ORGANIZATION, QUALIFIER};
use iced::widget::{button, column, container, pick_list, row, shader, text};
use iced::{
    window, Alignment, Application, Command, Element, Length, Settings, Subscription, Theme,
};
use std::time::Instant;

pub struct App {
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
pub enum Message {
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
                .height(Length::Fill)
                .width(Length::Fill);

            column![
                top_row,
                iced::widget::container::Container::new(shader)
                    .center_x()
                    .center_y(),
            ]
            .spacing(10)
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
