use crate::{gb_area, Scaling};
use iced::advanced::graphics::futures::event;
use iced::widget::{button, column, container, pick_list, shader, text};
use iced::{window, Alignment, Element, Length, Subscription, Theme};

#[derive(Debug, Clone)]
pub enum Message {
    ScalingChanged(Scaling),
    OpenButtonPressed,
    Tick,
    EventOcurred(iced::Event),
}

pub struct App {
    gb_area: gb_area::GbArea,
    _audio: ceres_audio::State,
    show_menu: bool,
    model: ceres_core::Model,
}

impl App {
    pub fn new(args: &crate::Cli) -> anyhow::Result<Self> {
        let audio = ceres_audio::State::new()?;
        Ok(App {
            gb_area: gb_area::GbArea::new(args.model.into(), args.file.as_deref(), &audio)?,
            _audio: audio,
            show_menu: false,
            model: args.model.into(),
        })
    }

    pub fn title(&self) -> String {
        "Ceres".to_owned()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ScalingChanged(scaling) => {
                self.gb_area.set_scaling(scaling);
            }
            Message::OpenButtonPressed => {
                let file = rfd::FileDialog::new()
                    .add_filter("gb", &["gb", "gbc"])
                    .pick_file();

                if let Some(file) = file {
                    match self.gb_area.change_rom(&file, self.model) {
                        Ok(_) => {
                            self.show_menu = false;
                        }
                        Err(e) => eprintln!("Error changing ROM: {e}"),
                    }
                }
            }
            Message::Tick => {
                // TODO: Why don't we need to do anything here?
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

    pub fn view(&self) -> Element<Message> {
        if self.show_menu {
            let content = column![
                text("Options").size(20),
                button("Open ROM")
                    .on_press(Message::OpenButtonPressed)
                    .padding(5),
                text("Scaling mode"),
                pick_list(
                    Scaling::ALL,
                    Some(self.gb_area.scaling()),
                    Message::ScalingChanged
                )
                .padding(5),
            ]
            .spacing(10);

            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into()
        } else {
            let shader = shader(self.gb_area.scene())
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
            window::frames().map(|_| Message::Tick),
            event::listen().map(Message::EventOcurred),
        ])
    }
}
