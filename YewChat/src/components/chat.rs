use serde::{Deserialize, Serialize};
use web_sys::{HtmlInputElement, KeyboardEvent};
use yew::prelude::*;
use yew_agent::{Bridge, Bridged};

use crate::{User, services::websocket::WebsocketService};
use crate::services::event_bus::EventBus;

pub enum Msg {
    HandleMsg(String),
    SubmitMessage,
    InputChanged,
    ToggleEmojiPicker,
    SelectEmoji(String),
    HandleKeyDown(KeyboardEvent),
}

#[derive(Deserialize, Clone)]
struct MessageData {
    from: String,
    message: String,
    timestamp: Option<String>, // Added timestamp field
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MsgTypes {
    Users,
    Register,
    Message,
    Typing, // Added typing message type
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebSocketMessage {
    message_type: MsgTypes,
    data_array: Option<Vec<String>>,
    data: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct TypingStatus {
    username: String,
    is_typing: bool,
}

#[derive(Clone)]
struct UserProfile {
    name: String,
    avatar: String,
}

pub struct Chat {
    users: Vec<UserProfile>,
    chat_input: NodeRef,
    wss: WebsocketService,
    messages: Vec<MessageData>,
    _producer: Box<dyn Bridge<EventBus>>,
    typing_users: Vec<String>,       // Added to track who's typing
    show_emoji_picker: bool,         // Added for emoji picker
    typing_timeout: Option<i32>,     // For debouncing typing events
}

impl Component for Chat {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let (user, _) = ctx
            .link()
            .context::<User>(Callback::noop())
            .expect("context to be set");
        let wss = WebsocketService::new();
        let username = user.username.borrow().clone();

        let message = WebSocketMessage {
            message_type: MsgTypes::Register,
            data: Some(username.to_string()),
            data_array: None,
        };

        if let Ok(_) = wss
            .tx
            .clone()
            .try_send(serde_json::to_string(&message).unwrap())
        {
            log::debug!("message sent successfully");
        }

        Self {
            users: vec![],
            messages: vec![],
            chat_input: NodeRef::default(),
            wss,
            _producer: EventBus::bridge(ctx.link().callback(Msg::HandleMsg)),
            typing_users: vec![],
            show_emoji_picker: false,
            typing_timeout: None,
        }
    }
    
    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::HandleMsg(s) => {
                let msg: WebSocketMessage = serde_json::from_str(&s).unwrap();
                match msg.message_type {
                    MsgTypes::Users => {
                        let users_from_message = msg.data_array.unwrap_or_default();
                        self.users = users_from_message
                            .iter()
                            .map(|u| UserProfile {
                                name: u.into(),
                                avatar: format!(
                                    "https://avatars.dicebear.com/api/adventurer-neutral/{}.svg",
                                    u
                                )
                                .into(),
                            })
                            .collect();
                        return true;
                    }
                    MsgTypes::Message => {
                        let message_data: MessageData =
                            serde_json::from_str(&msg.data.unwrap()).unwrap();
                        self.messages.push(message_data);
                        return true;
                    }
                    MsgTypes::Typing => {
                        // Handle typing status updates
                        if let Some(data) = msg.data {
                            let typing_status: TypingStatus = serde_json::from_str(&data).unwrap();
                            
                            if typing_status.is_typing {
                                // Add user to typing list if not already there
                                if !self.typing_users.contains(&typing_status.username) {
                                    self.typing_users.push(typing_status.username);
                                }
                            } else {
                                // Remove user from typing list
                                self.typing_users.retain(|u| u != &typing_status.username);
                            }
                            return true;
                        }
                        return false;
                    }
                    _ => {
                        return false;
                    }
                }
            }
            Msg::SubmitMessage => {
    let input = self.chat_input.cast::<HtmlInputElement>();
    if let Some(input) = input {
        let input_value = input.value();
        if !input_value.trim().is_empty() {
            // Send message without nesting
            let message = WebSocketMessage {
                message_type: MsgTypes::Message,
                data: Some(input_value),
                data_array: None,
            };
            
            if let Err(e) = self
                .wss
                .tx
                .clone()
                .try_send(serde_json::to_string(&message).unwrap())
            {
                log::debug!("error sending to channel: {:?}", e);
            }
            
            input.set_value("");
            self.send_typing_status(ctx, false);
        }
    };
    
    self.show_emoji_picker = false;
    true
}
            Msg::InputChanged => {
                // Send a typing status message
                self.send_typing_status(ctx, true);
                false
            }
            Msg::ToggleEmojiPicker => {
                self.show_emoji_picker = !self.show_emoji_picker;
                true
            }
            Msg::SelectEmoji(emoji) => {
                // Insert emoji at cursor position in input field
                if let Some(input) = self.chat_input.cast::<HtmlInputElement>() {
                    let current_value = input.value();
                    input.set_value(&format!("{}{}", current_value, emoji));
                    input.focus().unwrap();
                }
                false
            }
            Msg::HandleKeyDown(event) => {
                // Handle Enter key to submit
                if event.key() == "Enter" && !event.shift_key() {
                    event.prevent_default();
                    ctx.link().send_message(Msg::SubmitMessage);
                    return true;
                }
                false
            }
        }
    }
    
    fn view(&self, ctx: &Context<Self>) -> Html {
        let submit = ctx.link().callback(|_| Msg::SubmitMessage);
        let input_changed = ctx.link().callback(|_| Msg::InputChanged);
        let toggle_emoji = ctx.link().callback(|_| Msg::ToggleEmojiPicker);
        let on_keydown = ctx.link().callback(|e: KeyboardEvent| Msg::HandleKeyDown(e));
        
        // Create typing indicator text
        let typing_text = if !self.typing_users.is_empty() {
            if self.typing_users.len() == 1 {
                format!("{} is typing...", self.typing_users[0])
            } else if self.typing_users.len() == 2 {
                format!("{} and {} are typing...", self.typing_users[0], self.typing_users[1])
            } else {
                String::from("Several people are typing...")
            }
        } else {
            String::new()
        };
        
        html! {
            <div class="flex w-screen">
                <div class="flex-none w-56 h-screen bg-gray-100">
                    <div class="text-xl p-3">{"Users"}</div>
                    {
                        self.users.clone().iter().map(|u| {
                            html!{
                                <div class="flex m-3 bg-white rounded-lg p-2">
                                    <div>
                                        <img class="w-12 h-12 rounded-full" src={u.avatar.clone()} alt="avatar"/>
                                    </div>
                                    <div class="flex-grow p-3">
                                        <div class="flex text-xs justify-between">
                                            <div>{u.name.clone()}</div>
                                        </div>
                                        <div class="text-xs text-gray-400">
                                            {"Hi there!"}
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                </div>
                <div class="grow h-screen flex flex-col">
                    <div class="w-full h-14 border-b-2 border-gray-300">
                        <div class="text-xl p-3">{"💬 Chat!"}</div>
                    </div>
                    <div class="w-full grow overflow-auto border-b-2 border-gray-300">
                        {
                            self.messages.iter().map(|m| {
                                // Create the default profile outside the unwrap_or to avoid borrowing issues
                                let default_profile = UserProfile {
                                    name: m.from.clone(),
                                    avatar: format!(
                                        "https://avatars.dicebear.com/api/adventurer-neutral/{}.svg", 
                                        m.from
                                    ),
                                };
                                
                                // Now use the created profile
                                let user = self.users.iter().find(|u| u.name == m.from).unwrap_or(&default_profile);
                                
                                html!{
                                    <div class="flex items-end w-3/6 bg-gray-100 m-8 rounded-tl-lg rounded-tr-lg rounded-br-lg">
                                        <img class="w-8 h-8 rounded-full m-3" src={user.avatar.clone()} alt="avatar"/>
                                        <div class="p-3 w-full">
                                            <div class="flex justify-between items-center">
                                                <div class="text-sm font-medium">
                                                    {m.from.clone()}
                                                </div>
                                                <div class="text-xs text-gray-400">
                                                    {m.timestamp.clone().unwrap_or_default()}
                                                </div>
                                            </div>
                                            <div class="text-xs text-gray-700 mt-1">
                                                {
                                                    if m.message.ends_with(".gif") {
                                                        html! {
                                                            <img class="mt-3" src={m.message.clone()}/>
                                                        }
                                                    } else {
                                                        html! {
                                                            {m.message.clone()}
                                                        }
                                                    }
                                                }
                                            </div>
                                        </div>
                                    </div>
                                }
                            }).collect::<Html>()
                        }
                        
                        {
                            // Display typing indicators
                            if !self.typing_users.is_empty() {
                                html! {
                                    <div class="flex items-center px-6 py-2 text-sm italic text-gray-500">
                                        {typing_text}
                                        <div class="flex items-center ml-2">
                                            <div class="w-2 h-2 bg-gray-400 rounded-full mr-1 animate-bounce"></div>
                                            <div class="w-2 h-2 bg-gray-400 rounded-full mr-1 animate-bounce"></div>
                                            <div class="w-2 h-2 bg-gray-400 rounded-full animate-bounce"></div>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <div class="w-full h-14 flex px-3 items-center relative">
                        <button 
                            onclick={toggle_emoji}
                            class="p-2 text-gray-500 hover:text-gray-700 focus:outline-none"
                        >
                            {"😀"}
                        </button>
                        <input 
                            ref={self.chat_input.clone()} 
                            type="text" 
                            placeholder="Message" 
                            class="block w-full py-2 pl-4 mx-3 bg-gray-100 rounded-full outline-none focus:text-gray-700" 
                            name="message" 
                            onkeydown={on_keydown}
                            oninput={input_changed}
                            required=true 
                        />
                        <button 
                            onclick={submit} 
                            class="p-3 shadow-sm bg-blue-600 w-10 h-10 rounded-full flex justify-center items-center color-white"
                        >
                            <svg fill="#000000" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" class="fill-white">
                                <path d="M0 0h24v24H0z" fill="none"></path><path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"></path>
                            </svg>
                        </button>
                        
                        {
                            // Emoji picker
                            if self.show_emoji_picker {
                                let emojis = vec!["😀", "😂", "😍", "🥳", "😎", "🤔", "👍", "❤️", "🎉", "🔥", "👏", "✅", "🙏", "🤣", "😊", "🥰"];
                                
                                html! {
                                    <div class="absolute bottom-16 left-4 bg-white shadow-lg rounded-lg p-2 grid grid-cols-8 gap-1 z-10">
                                        {
                                            emojis.iter().map(|emoji| {
                                                let emoji_clone = emoji.to_string();
                                                let onclick = ctx.link().callback(move |_| Msg::SelectEmoji(emoji_clone.clone()));
                                                
                                                html! {
                                                    <button onclick={onclick} class="p-1 text-xl hover:bg-gray-100 rounded">
                                                        {emoji}
                                                    </button>
                                                }
                                            }).collect::<Html>()
                                        }
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                </div>
            </div>
        }
    }
}

impl Chat {
    fn send_typing_status(&mut self, ctx: &Context<Self>, is_typing: bool) {
        // Get current user
        let (user, _) = ctx
            .link()
            .context::<User>(Callback::noop())
            .expect("context to be set");
        
        let username = user.username.borrow().clone();
        
        // Create typing status
        let typing_status = TypingStatus {
            username,
            is_typing,
        };
        
        // Send typing status through WebSocket
        let message = WebSocketMessage {
            message_type: MsgTypes::Typing,
            data: Some(serde_json::to_string(&typing_status).unwrap()),
            data_array: None,
        };
        
        if let Err(e) = self
            .wss
            .tx
            .clone()
            .try_send(serde_json::to_string(&message).unwrap())
        {
            log::debug!("error sending typing status: {:?}", e);
        }
    }
}