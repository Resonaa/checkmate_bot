use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs;

pub mod bot;
pub mod event;
pub mod map;

#[macro_use]
extern crate log;

#[derive(Deserialize)]
pub struct Config {
    pub cookie_1: String,
    pub cookie_2: String,
    pub room: String,
    pub map: u8,
    pub speed: u8,
    pub private: bool,
    pub bot_uid: Vec<u32>,
    pub leaderboard_id: u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Upgrader,
    Player,
}

lazy_static! {
    pub static ref CONFIG: Config =
        toml::from_str(&fs::read_to_string("config.toml").unwrap()).unwrap();
}
