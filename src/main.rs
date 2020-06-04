use iced::{Application, Settings};

mod account;
mod app;
mod chat;
mod chat_list;

fn main() {
    femme::start();

    crate::app::App::run(Settings::default())
}
