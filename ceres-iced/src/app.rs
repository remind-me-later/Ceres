use crate::{gb_widget, Scaling, CERES_STYLIZED, ORGANIZATION, QUALIFIER};
use iced::advanced::graphics::futures::event;
use iced::widget::{button, column, container, pick_list, shader, text};
use iced::{window, Alignment, Element, Length, Subscription, Theme};
use std::time::Instant;

pub struct App {
    widget: gb_widget::GbWidget,
    _audio: ceres_audio::State,
    show_menu: bool,
    project_dirs: directories::ProjectDirs,
    model: ceres_core::Model,
}

impl Default for App {
    fn default() -> Self {
        let project_dirs =
            directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, CERES_STYLIZED).unwrap();
        let audio = ceres_audio::State::new().unwrap();
        let model = ceres_core::Model::Cgb;

        App {
            widget: gb_widget::GbWidget::new(model, &project_dirs, None, &audio).unwrap(),
            _audio: audio,
            project_dirs,
            show_menu: false,
            model,
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
    pub fn title(&self) -> String {
        "Ceres".to_owned()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ScalingChanged(scaling) => {
                self.widget.set_scaling(scaling);
            }
            Message::OpenButtonPressed => {
                let file = rfd::FileDialog::new()
                    .add_filter("gb", &["gb", "gbc"])
                    .pick_file();

                if let Some(file) = file {
                    match self
                        .widget
                        .change_rom(&self.project_dirs, &file, self.model)
                    {
                        Ok(_) => {}
                        Err(e) => eprintln!("Error changing ROM: {}", e),
                    }
                }
            }
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
            let content = column![
                text("Options").size(20),
                button("Open ROM").on_press(Message::OpenButtonPressed),
                text("Scaling mode"),
                pick_list(
                    Scaling::ALL,
                    Some(self.widget.scaling()),
                    Message::ScalingChanged
                )
            ]
            .spacing(10);

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
