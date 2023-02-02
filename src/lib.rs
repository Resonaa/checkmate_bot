use consts::default_calc_cnt;
use indexmap::IndexSet;
use serde::Deserialize;
use std::collections::HashMap;

mod bot;
pub mod consts;
mod event;
mod map;
pub mod socket;

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

    #[serde(default = "default_calc_cnt")]
    pub calc_cnt: u8,
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
    pub rooms: HashMap<&'a str, RoomConfig>,
}

#[derive(Clone)]
pub struct BotData {
    pub id: usize,
    pub bot: BotConfig<'static>,
    pub team: IndexSet<u32>,
    pub room: Option<RoomConfig>,
}
