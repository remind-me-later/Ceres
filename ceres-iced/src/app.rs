use crate::{audio, gb_widget, Scaling, CERES_STYLIZED, ORGANIZATION, QUALIFIER};
use iced::advanced::graphics::futures::event;
use iced::widget::shader::Program;
use iced::widget::{button, column, container, pick_list, row, shader, text};
use iced::{window, Alignment, Application, Element, Length, Settings, Subscription, Theme};
use std::time::Instant;

pub struct App {
    widget: gb_widget::GbWidget,
    _audio: audio::State,
    show_menu: bool,
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
            show_menu: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ScalingChanged(Scaling),
    OpenButtonPressed,
    ExportButtonPressed,
    Tick(Instant),
    EventOcurred(iced::Event),
    MenuToggled,
}

impl App {
    pub fn new() -> Self {
        App::default()
    }

    pub fn title(&self) -> String {
        String::from("Ceres")
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ScalingChanged(scaling) => {
                self.widget.set_scaling(scaling);
            }
            Message::OpenButtonPressed => {}
            Message::ExportButtonPressed => {}
            Message::Tick(_) => {
                // self.widget.update();
            }
            Message::MenuToggled => {
                self.show_menu = !self.show_menu;
            }
            Message::EventOcurred(event) => {
                if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                    ..
                }) = event
                {
                    self.show_menu = !self.show_menu;
                }
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        if self.show_menu {
            let content = {
                column![
                    text("Scaling mode"),
                    pick_list(
                        Scaling::ALL,
                        Some(self.widget.scaling()),
                        Message::ScalingChanged
                    )
                    .width(Length::Shrink)
                ]
            };

            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
        } else {
            let shader = shader(self.widget.scene())
                .height(Length::Fill)
                .width(Length::Fill);

            container(shader)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::GruvboxLight
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // window::frames().map(Message::Tick)
        iced::Subscription::batch(vec![
            window::frames().map(Message::Tick),
            event::listen().map(Message::EventOcurred),
        ])
    }
}
