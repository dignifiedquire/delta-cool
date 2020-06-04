use super::account;
use iced::{
    button, image, Background, Color, Column, Element, HorizontalAlignment, Length, Row, Space,
    Text,
};

#[derive(Debug, Clone)]
pub struct ChatMsg {
    pub msg: account::ChatMessage,
    pub button_state: button::State,
}

#[derive(Debug, Clone)]
pub enum ChatMsgMessage {}

impl ChatMsg {
    pub fn new(msg: account::ChatMessage) -> Self {
        Self {
            msg,
            button_state: Default::default(),
        }
    }

    pub fn view(&mut self) -> Element<ChatMsgMessage> {
        let row = Row::new().spacing(20);

        let row = if self.msg.is_info {
            // Info messages
            row.push(
                Text::new(self.msg.text.as_ref().cloned().unwrap_or_default())
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .size(16)
                    .color([0.5, 0.5, 0.5])
                    .width(Length::Fill),
            )
        } else {
            // Regular Messages
            let row = if let Some(img) = &self.msg.from_profile_image {
                let img = image::Image::new(img)
                    .width(Length::Units(40))
                    .height(Length::Units(40));
                row.push(img)
            } else {
                row.push(Space::new(Length::Units(40), Length::Units(40)))
            };

            row.push(
                Column::new()
                    .spacing(5)
                    .push(
                        Row::new()
                            .spacing(10)
                            .push(
                                Text::new(self.msg.from_first_name.clone())
                                    .horizontal_alignment(HorizontalAlignment::Left)
                                    .size(18)
                                    .color(Color::BLACK)
                                    .width(Length::Fill),
                            )
                            .push(
                                Text::new(self.msg.timestamp.lazy_format("%r").to_string())
                                    .horizontal_alignment(HorizontalAlignment::Left)
                                    .size(16)
                                    .color([0.5, 0.5, 0.5])
                                    .width(Length::Fill),
                            ),
                    )
                    .push(
                        Text::new(self.msg.text.as_ref().cloned().unwrap_or_default())
                            .horizontal_alignment(HorizontalAlignment::Left)
                            .size(18)
                            .color(Color::BLACK)
                            .width(Length::Fill),
                    ),
            )
        };

        struct Style {}
        impl button::StyleSheet for Style {
            fn active(&self) -> button::Style {
                button::Style {
                    background: Some(Background::Color(Color::WHITE)),
                    text_color: Color::BLACK,
                    ..button::Style::default()
                }
            }
        }

        button::Button::new(&mut self.button_state, row)
            .style(Style {})
            .into()
    }
}
