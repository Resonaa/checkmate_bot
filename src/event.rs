use crate::map;
use anyhow::Result;
use rust_socketio::{Payload, RawClient};
use serde::Deserialize;

pub fn callback<T, R>(mut input: T) -> impl FnMut(Payload, RawClient) + 'static + Sync + Send
where
    T: FnMut(String, RawClient) -> R + 'static + Sync + Send,
    R: Into<Result<()>>,
{
    move |payload, socket| {
        if let Payload::String(s) = payload {
            if let Err(err) = input(s, socket).into() {
                error!("{:?}", err);
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Speed<'a> {
    U8(u8),
    String(&'a str),
}

#[derive(Deserialize)]
pub struct UpdateSettings<'a> {
    #[serde(borrow)]
    pub speed: Speed<'a>,

    pub private: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum NewMapNode {
    MapInfo(map::MapInfo),
    Land(map::Land),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum MapUpdate {
    Round(u32),
    Data(Vec<[String; 3]>),
}
