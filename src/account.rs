use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use async_std::sync::{Arc, RwLock};
use deltachat::{
    chat::{self, Chat, ChatId},
    chatlist::Chatlist,
    constants::{Chattype, Viewtype},
    contact::Contact,
    context::Context,
    message::{self, MessageState, MsgId},
    EventEmitter,
};
use lazy_static::lazy_static;
use log::*;
use time::OffsetDateTime;

lazy_static! {
    pub static ref HOME_DIR: PathBuf = dirs::home_dir()
        .unwrap_or_else(|| "home".into())
        .join(".deltachat");
}

#[derive(Debug, Clone)]
pub struct Account {
    pub context: Context,
    pub state: Arc<RwLock<AccountState>>,
}

#[derive(Debug, Clone)]
pub struct AccountState {
    pub logged_in: bool,
    pub email: String,
    pub chat_states: BTreeMap<ChatId, ChatState>,
    pub selected_chat: Option<ChatState>,
    pub selected_chat_id: Option<ChatId>,
    pub chatlist: Chatlist,
    /// Messages of the selected chat
    pub chat_msg_ids: Vec<MsgId>,
    /// State of currently selected chat messages
    pub chat_msgs: BTreeMap<usize, ChatMessage>,
    chat_msgs_range: (usize, usize),
    /// indexed by index in the Chatlist
    pub chats: BTreeMap<ChatId, Chat>,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub id: MsgId,
    pub from_id: u32,
    pub from_first_name: String,
    pub from_profile_image: Option<PathBuf>,
    pub from_color: u32,
    pub viewtype: Viewtype,
    pub state: MessageState,
    pub text: Option<String>,
    pub starred: bool,
    pub timestamp: OffsetDateTime,
    pub is_info: bool,
    pub file: Option<PathBuf>,
    pub file_height: i32,
    pub file_width: i32,
}

#[derive(Debug, Clone)]
pub struct ChatState {
    pub index: Option<usize>,
    pub id: ChatId,
    pub name: String,
    pub header: String,
    pub preview: String,
    pub timestamp: OffsetDateTime,
    pub state: String,
    pub profile_image: Option<PathBuf>,
    pub fresh_msg_cnt: usize,
    pub can_send: bool,
    pub is_self_talk: bool,
    pub is_device_talk: bool,
    pub chat_type: Chattype,
    pub color: u32,
}

impl Account {
    pub async fn new(email: &str) -> Result<Self> {
        // TODO: escape email to be a vaild filesystem name
        let path = HOME_DIR.join(format!("{}.sqlite", email));

        // Ensure the folders actually exist
        if let Some(parent) = path.parent() {
            async_std::fs::create_dir_all(parent).await?;
        }

        let context = Context::new("desktop".into(), path.into())
            .await
            .map_err(|err| anyhow!("{:?}", err))?;

        context.start_io().await;

        let chatlist = Chatlist::try_load(&context, 0, None, None)
            .await
            .map_err(|err| anyhow!("failed to load chats: {:?}", err))?;

        let mut account = Account {
            context,
            state: Arc::new(RwLock::new(AccountState {
                logged_in: true,
                email: email.to_string(),
                chats: Default::default(),
                selected_chat: None,
                selected_chat_id: None,
                chatlist,
                chat_msgs_range: (0, 0),
                chat_msg_ids: Default::default(),
                chat_msgs: Default::default(),
                chat_states: Default::default(),
            })),
        };

        account.load_chat_list().await?;

        Ok(account)
    }

    pub async fn logged_in(&self) -> bool {
        self.state.read().await.logged_in
    }

    pub fn get_event_emitter(&self) -> EventEmitter {
        self.context.get_event_emitter()
    }

    pub async fn import(&self, path: &str) -> Result<()> {
        use deltachat::imex;

        imex::imex(&self.context, imex::ImexMode::ImportBackup, Some(path)).await?;

        // TODO: start_io

        Ok(())
    }

    pub async fn login(&mut self, email: &str, password: &str) -> Result<()> {
        use deltachat::config::Config;
        self.context.set_config(Config::Addr, Some(email)).await?;
        self.context
            .set_config(Config::MailPw, Some(password))
            .await?;

        self.configure().await?;
        self.state.write().await.logged_in = true;

        Ok(())
    }

    pub async fn configure(&self) -> Result<()> {
        info!("configure");
        self.context.configure().await?;
        Ok(())
    }

    pub async fn load_chat_list(&mut self) -> Result<()> {
        let state = &mut *self.state.write().await;
        state.chat_states.clear();

        for i in 0..state.chatlist.len() {
            let chat_id = state.chatlist.get_chat_id(i);
            refresh_chat_state(self.context.clone(), state, chat_id).await?;
        }

        Ok(())
    }

    pub async fn select_chat(&mut self, chat_id: ChatId) -> Result<()> {
        info!("selecting chat {:?}", chat_id);
        let state = &mut *self.state.write().await;
        let (chat, chat_state) = load_chat_state(self.context.clone(), state, chat_id).await?;

        state.selected_chat_id = Some(chat_id);
        state.chat_msg_ids = chat::get_chat_msgs(&self.context, chat_id, 0, None).await;
        state.chat_msgs = Default::default();

        // mark as noticed
        chat::marknoticed_chat(&self.context, chat_id)
            .await
            .map_err(|err| anyhow!("failed to mark noticed: {:?}", err))?;

        if let Some(chat_state) = chat_state {
            state.selected_chat = Some(chat_state);
        }

        state.chats.insert(chat.id, chat);

        Ok(())
    }

    pub async fn load_message_list(&mut self) -> Result<()> {
        let state = &mut *self.state.write().await;

        refresh_message_list(self.context.clone(), state, None).await?;

        // markseen messages that we load
        // could be better, by checking actual in view, but close enough for now
        let msgs_list = state.chat_msg_ids.clone();
        message::markseen_msgs(&self.context, msgs_list).await;

        Ok(())
    }

    pub async fn send_text_message(&self, text: String) -> Result<()> {
        if let Some(chat_id) = self.state.read().await.selected_chat_id {
            chat::send_text_msg(&self.context, chat_id, text)
                .await
                .map_err(|err| anyhow!("failed to send message: {}", err))?;
        } else {
            bail!("no chat selected, can not send message");
        }

        Ok(())
    }

    pub async fn send_file_message(
        &self,
        typ: Viewtype,
        path: String,
        text: Option<String>,
        mime: Option<String>,
    ) -> Result<()> {
        if let Some(chat_id) = self.state.read().await.selected_chat_id {
            let mut msg = message::Message::new(typ);
            msg.set_text(text);
            msg.set_file(path, mime.as_deref());

            chat::send_msg(&self.context, chat_id, &mut msg)
                .await
                .map_err(|err| anyhow!("failed to send message: {}", err))?;
        } else {
            bail!("no chat selected, can not send message");
        }

        Ok(())
    }

    pub async fn create_chat_by_id(&self, id: MsgId) -> Result<ChatId> {
        let chat = chat::create_by_msg_id(&self.context, id)
            .await
            .map_err(|err| anyhow!("failed to create chat: {}", err))?;

        // TODO: select that chat?
        Ok(chat)
    }

    pub async fn maybe_network(&self) {
        self.context.maybe_network().await;
    }
}

pub async fn refresh_chat_state(
    context: Context,
    state: &mut AccountState,
    chat_id: ChatId,
) -> Result<()> {
    info!("refreshing chat state: {:?}", &chat_id);

    let (chat, chat_state) = load_chat_state(context, state, chat_id).await?;

    if let Some(chat_state) = chat_state {
        if let Some(sel_chat_id) = state.selected_chat_id {
            if sel_chat_id == chat_id {
                state.selected_chat = Some(chat_state.clone());
            }
        }

        if chat_state.index.is_some() {
            // Only insert if there is actually a valid index.
            state.chat_states.insert(chat_id, chat_state);
        }
    }
    state.chats.insert(chat.id, chat);

    Ok(())
}

async fn load_chat_state(
    context: Context,
    state: &AccountState,
    chat_id: ChatId,
) -> Result<(Chat, Option<ChatState>)> {
    let chats = &state.chatlist;
    let chat = Chat::load_from_db(&context, chat_id)
        .await
        .map_err(|err| anyhow!("failed to load chats: {:?}", err))?;

    let chat_state = if let Some(index) = chats.get_index_for_id(chat_id) {
        let lot = chats.get_summary(&context, index, Some(&chat)).await;

        let header = lot.get_text1().map(|s| s.to_string()).unwrap_or_default();
        let preview = lot.get_text2().map(|s| s.to_string()).unwrap_or_default();

        let index = state.chatlist.get_index_for_id(chat_id);

        Some(ChatState {
            id: chat_id,
            index,
            name: chat.get_name().to_string(),
            header,
            preview,
            timestamp: OffsetDateTime::from_unix_timestamp(lot.get_timestamp()),
            state: lot.get_state().to_string(),
            profile_image: chat.get_profile_image(&context).await.map(Into::into),
            can_send: chat.can_send(),
            chat_type: chat.get_type(),
            color: chat.get_color(&context).await,
            is_device_talk: chat.is_device_talk(),
            is_self_talk: chat.is_self_talk(),
            fresh_msg_cnt: chat_id.get_fresh_msg_cnt(&context).await,
        })
    } else {
        None
    };

    Ok((chat, chat_state))
}

pub async fn refresh_chat_list(context: Context, state: &mut AccountState) -> Result<()> {
    let chatlist = Chatlist::try_load(&context, 0, None, None)
        .await
        .map_err(|err| anyhow!("failed to load chats: {:?}", err))?;

    state.chatlist = chatlist;

    Ok(())
}

pub async fn refresh_message_list(
    context: Context,
    state: &mut AccountState,
    chat_id: Option<ChatId>,
) -> Result<()> {
    let current_chat_id = state.selected_chat_id.clone();
    if chat_id.is_some() && current_chat_id != chat_id {
        return Ok(());
    }
    if current_chat_id.is_none() {
        // Ignore if no chat is selected
        return Ok(());
    }

    info!("loading chat messages {:?}", chat_id,);

    state.chat_msg_ids = chat::get_chat_msgs(&context, current_chat_id.unwrap(), 0, None).await;

    let mut msgs = BTreeMap::new();
    for (i, msg_id) in state.chat_msg_ids.iter().enumerate() {
        let msg = message::Message::load_from_db(&context, *msg_id)
            .await
            .map_err(|err| anyhow!("failed to load msg: {}: {}", msg_id, err))?;

        let from = Contact::load_from_db(&context, msg.get_from_id())
            .await
            .map_err(|err| anyhow!("failed to load contact: {}: {}", msg.get_from_id(), err))?;

        let chat_msg = ChatMessage {
            id: msg.get_id(),
            from_id: msg.get_from_id(),
            viewtype: msg.get_viewtype(),
            from_first_name: from.get_first_name().to_string(),
            from_profile_image: from.get_profile_image(&context).await.map(Into::into),
            from_color: from.get_color(),
            starred: msg.is_starred(),
            state: msg.get_state(),
            text: msg.get_text(),
            timestamp: OffsetDateTime::from_unix_timestamp(msg.get_sort_timestamp()),
            is_info: msg.is_info(),
            file: msg.get_file(&context).map(Into::into),
            file_width: msg.get_width(),
            file_height: msg.get_height(),
        };
        msgs.insert(i, chat_msg);
    }

    state.chat_msgs = msgs;

    Ok(())
}
