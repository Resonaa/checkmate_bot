use anyhow::Result;
use checkmate_bot::{bot::new_bot, BotData, Config};
use indexmap::IndexSet;
use lazy_static::lazy_static;
use log::info;
use regex::Regex;
use std::{collections::HashMap, fs, thread};

lazy_static! {
    static ref CONFIG: String = fs::read_to_string("config.toml").unwrap();
    static ref BOT_DATA: Vec<BotData> = {
        (|| -> Result<_> {
            let config: Config = toml::from_str(&CONFIG)?;

            let mut ans = Vec::new();

            let uid = {
                let mut uid = Vec::new();

                let client = reqwest::blocking::Client::new();

                let re = Regex::new(r"/user/(\d*)")?;

                for (id, bot) in config.bots.iter().enumerate() {
                    let res = client
                        .get("https://kana.byha.top:444/checkmate/room")
                        .header("cookie", bot.cookie)
                        .send()
                        .and_then(|res| res.text())?;

                    match re.captures(&res) {
                        Some(caps) => uid.push(caps.get(1).unwrap().as_str().parse()?),
                        None => panic!("cookie No.{} has expired", id + 1),
                    }
                }

                uid
            };

            let mut bot_in_room: HashMap<String, Vec<u32>> = HashMap::new();

            let mut priority = Vec::new();

            for (id, bot) in config.bots.iter().enumerate() {
                let vec = bot_in_room
                    .entry(format!("Room {} Team {}", bot.room, bot.team))
                    .or_default();

                vec.push(uid[id]);

                priority.push(vec.len());
            }

            for (id, bot) in config.bots.into_iter().enumerate() {
                let vec = bot_in_room
                    .get(&format!("Room {} Team {}", bot.room, bot.team))
                    .unwrap()
                    .to_owned();

                ans.push(BotData {
                    id: priority[id],
                    bot,
                    team: IndexSet::from_iter(vec),
                    room: config.rooms.get(&bot.room).copied(),
                });
            }

            info!("{:?}", bot_in_room);

            Ok(ans)
        })()
        .unwrap()
    };
}

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    for bot_data in BOT_DATA.iter() {
        new_bot(bot_data)?;
    }

    loop {
        thread::park();
    }
}
