use indexmap::IndexSet;
use serde::Deserialize;
use std::collections::HashMap;

pub mod bot;
pub mod event;
pub mod map;

#[macro_use]
extern crate log;

#[derive(Deserialize, Clone, Copy)]
#[serde(untagged)]
pub enum AutoReady {
    Unconditional(bool),
    Conditional { more_than: u8 },
}

#[derive(Deserialize, Clone, Copy)]
pub struct BotConfig<'a> {
    pub cookie: &'a str,
    pub room: &'a str,
    pub auto_ready: AutoReady,

    #[serde(default)]
    pub team: u32,
}

#[derive(Deserialize, Clone, Copy)]
pub struct RoomConfig {
    pub map: Option<u8>,
    pub speed: Option<u8>,
    pub private: Option<bool>,
}

#[derive(Deserialize)]
pub struct Config<'a> {
    #[serde(borrow)]
    pub bots: Vec<BotConfig<'a>>,

    #[serde(borrow)]
    pub rooms: HashMap<&'a str, RoomConfig>,
}

pub struct BotData {
    pub id: usize,
    pub bot: BotConfig<'static>,
    pub team: IndexSet<u32>,
    pub room: Option<RoomConfig>,
}
