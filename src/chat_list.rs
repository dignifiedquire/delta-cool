use std::path::PathBuf;

use iced::{button, image, Color, Column, Element, HorizontalAlignment, Length, Row, Space, Text};

use super::{account, chat::ChatMsg};

#[derive(Debug, Clone)]
pub struct ChatListEntry {
    pub id: deltachat::chat::ChatId,
    pub name: String,
    pub preview: String,
    pub profile_image: Option<PathBuf>,
    pub button_state: button::State,
}

#[derive(Debug, Clone)]
pub enum ChatListEntryMessage {
    Select,
    Selected(Vec<ChatMsg>, String),
}

impl ChatListEntry {
    pub fn new(chat: &account::ChatState) -> Self {
        Self {
            id: chat.id,
            name: chat.name.clone(),
            preview: chat.preview.clone(),
            profile_image: chat.profile_image.clone(),
            button_state: Default::default(),
        }
    }

    pub fn view(&mut self) -> Element<ChatListEntryMessage> {
        let row = Row::new().spacing(20);
        let row = if let Some(img) = &self.profile_image {
            let img = image::Image::new(img)
                .width(Length::Units(60))
                .height(Length::Units(60));
            row.push(img)
        } else {
            row.push(Space::new(Length::Units(60), Length::Units(60)))
        };

        let row = row.push(
            Column::new()
                .spacing(5)
                .max_width(250)
                .max_height(60)
                .push(
                    Text::new(self.name.clone())
                        .color(Color::BLACK)
                        .horizontal_alignment(HorizontalAlignment::Left)
                        .size(18)
                        .width(Length::Fill),
                )
                .push(
                    Text::new(self.preview.clone())
                        .color([0.5, 0.5, 0.5])
                        .horizontal_alignment(HorizontalAlignment::Left)
                        .size(16)
                        .width(Length::Fill),
                ),
        );

        button::Button::new(&mut self.button_state, row)
            .on_press(ChatListEntryMessage::Select)
            .into()
    }
}
