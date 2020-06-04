use iced::{
    scrollable, Application, Color, Column, Command, Container, Element, HorizontalAlignment,
    Length, Row, Scrollable, Subscription, Text,
};
use log::{error, info};

use crate::account::Account;
use crate::chat::*;
use crate::chat_list::*;

#[derive(Debug)]
pub enum App {
    Loading,
    Loaded(State),
}

#[derive(Debug, Clone)]
pub struct State {
    account: Account,
    scroll_chat: scrollable::State,
    scroll_chat_list: scrollable::State,
    chat_list: Vec<ChatListEntry>,
    chat: Vec<ChatMsg>,
    chat_name: String,
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(std::result::Result<State, String>),
    Event(deltachat::Event),
    ChatListEntryMessage(deltachat::chat::ChatId, ChatListEntryMessage),
    ChatMessage(deltachat::message::MsgId, ChatMsgMessage),
}

impl Application for App {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            App::Loading,
            Command::perform(load_state(), Message::Loaded),
        )
    }

    fn title(&self) -> String {
        "delta.cool".into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        match self {
            App::Loaded(state) => {
                Subscription::from_recipe(EventSubscription::new(state.account.get_event_emitter()))
                    .map(Message::Event)
            }
            _ => Subscription::none(),
        }
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Loaded(Ok(state)) => {
                *self = App::Loaded(state);
            }
            Message::Event(ev) => {
                info!("{:?}", ev);
            }
            Message::ChatListEntryMessage(id, msg) => match msg {
                ChatListEntryMessage::Select => {
                    if let App::Loaded(State { account, .. }) = self {
                        let mut account = account.clone();
                        let id = id.clone();
                        return Command::perform(
                            async move {
                                account.select_chat(id).await.unwrap();
                                account.load_message_list().await.unwrap();
                                let chat = account
                                    .state
                                    .read()
                                    .await
                                    .chat_msgs
                                    .iter()
                                    .map(|(_, msg)| ChatMsg::new(msg.clone()))
                                    .collect::<Vec<_>>();

                                (
                                    chat,
                                    account
                                        .state
                                        .read()
                                        .await
                                        .selected_chat
                                        .as_ref()
                                        .map(|s| s.name.clone())
                                        .unwrap_or_default(),
                                )
                            },
                            move |(chat, name)| {
                                Message::ChatListEntryMessage(
                                    id,
                                    ChatListEntryMessage::Selected(chat, name),
                                )
                            },
                        );
                    }
                }
                ChatListEntryMessage::Selected(new_chat, new_chat_name) => {
                    info!("selected chat: {}", id);
                    if let App::Loaded(State {
                        chat, chat_name, ..
                    }) = self
                    {
                        *chat = new_chat;
                        *chat_name = new_chat_name;
                    }
                }
            },
            Message::ChatMessage(_, _) => {}
            Message::Loaded(Err(err)) => {
                // TODO: proper error handling
                error!("{}", err);
            }
        }
        Command::none()
    }

    fn view(&mut self) -> Element<Self::Message> {
        if let App::Loaded(State {
            scroll_chat,
            scroll_chat_list,
            chat_list,
            chat,
            chat_name,
            ..
        }) = self
        {
            let chats: Element<_> = chat_list
                .iter_mut()
                .fold(Column::new().spacing(5), |column, entry| {
                    let id = entry.id.clone();
                    column.push(
                        entry
                            .view()
                            .map(move |message| Message::ChatListEntryMessage(id, message)),
                    )
                })
                .into();
            let chat_el: Element<_> = chat
                .iter_mut()
                .fold(Column::new().spacing(2), |column, entry| {
                    let id = entry.msg.id.clone();
                    column.push(
                        entry
                            .view()
                            .map(move |message| Message::ChatMessage(id, message)),
                    )
                })
                .into();

            let content = Column::new().push(
                Row::new()
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .push(
                        Column::new()
                            .max_width(300)
                            .spacing(10)
                            .push(Scrollable::new(scroll_chat_list).padding(40).push(chats)),
                    )
                    .push(
                        Column::new()
                            .push(Text::new(chat_name.clone()).color(Color::BLACK).size(20))
                            .push(Scrollable::new(scroll_chat).padding(10).push(chat_el)),
                    ),
            );

            Container::new(content)
                .width(Length::Fill)
                .center_x()
                .into()
        } else {
            Container::new(
                Text::new("Welcome to delta.cool")
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .size(50),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_y()
            .into()
        }
    }
}

async fn load_state() -> std::result::Result<State, String> {
    let account = Account::new("d@testrun.org")
        .await
        .map_err(|err| err.to_string())?;

    let chat_list = account
        .state
        .read()
        .await
        .chat_states
        .iter()
        .map(|(_, chat)| ChatListEntry::new(chat))
        .collect();

    Ok(State {
        account,
        scroll_chat: Default::default(),
        scroll_chat_list: Default::default(),
        chat_list,
        chat: Default::default(),
        chat_name: Default::default(),
    })
}

struct EventSubscription {
    events: Option<deltachat::EventEmitter>,
}

impl EventSubscription {
    fn new(events: deltachat::EventEmitter) -> Self {
        EventSubscription {
            events: Some(events),
        }
    }
}

// Make sure iced can use our download stream
impl<H, I> iced_native::subscription::Recipe<H, I> for EventSubscription
where
    H: std::hash::Hasher,
{
    type Output = deltachat::Event;

    fn hash(&self, state: &mut H) {
        use std::hash::Hash;

        std::any::TypeId::of::<Self>().hash(state);
    }

    fn stream(
        mut self: Box<Self>,
        _input: futures::stream::BoxStream<'static, I>,
    ) -> futures::stream::BoxStream<'static, Self::Output> {
        Box::pin(futures::stream::unfold(
            self.events.take().expect("missing events"),
            |events| async move { events.recv().await.map(|ev| (ev, events)) },
        ))
    }
}
