use crate::{
    bot::Bot,
    consts::WS_URL,
    event::{self, callback},
    AutoReady, BotData,
};
use anyhow::Result;
use parking_lot::Mutex;
use rust_socketio::{ClientBuilder, RawClient};
use serde_json::json;
use std::sync::Arc;

fn vote_start(socket: &RawClient, config: &BotData) -> Result<()> {
    if let Some(room_config) = config.room {
        if let Some(map) = room_config.map {
            socket.emit("changeSettings", json!({"map": map.to_string()}))?;
        }
    }

    if let AutoReady::Unconditional(true) = config.bot.auto_ready {
        socket.emit("VoteStart", json!(1))?;
    }

    Ok(())
}

pub fn new_bot(config: &'static BotData) -> Result<()> {
    let global_bot = Arc::new(Mutex::new(Bot::new(config)));
    let global_is_ready = Arc::new(Mutex::new(false));

    let open = move |_, socket: RawClient| {
        info!("{} connected", config.team[config.id - 1]);
        socket.emit("joinRoom", config.bot.room)?;

        vote_start(&socket, config)
    };

    let update_settings = |payload: String, socket: RawClient| {
        let update_settings: event::UpdateSettings = serde_json::from_str(&payload)?;

        if let Some(room_config) = config.room {
            if let Some(config_speed) = room_config.speed {
                match update_settings.speed {
                    event::Speed::U8(speed) => {
                        if speed != config_speed {
                            socket.emit("changeSettings", json!({ "speed": config_speed }))?;
                        }
                    }
                    event::Speed::String(speed) => {
                        if speed != config_speed.to_string() {
                            socket.emit("changeSettings", json!({ "speed": config_speed }))?;
                        }
                    }
                }
            }

            if let Some(config_private) = room_config.private {
                if update_settings.private != config_private {
                    socket.emit("changeSettings", json!({ "private": config_private }))?;
                }
            }
        }

        Ok(())
    };

    let bot = global_bot.clone();
    let update_gm = move |payload: String, _| {
        use event::NewMapNode;

        let update_gm: Vec<Vec<_>> = serde_json::from_str(&payload)?;

        let mut bot = bot.lock();

        if let NewMapNode::MapInfo(map_info) = &update_gm[0][0] {
            bot.size = map_info.size;
        }

        bot.gm = update_gm
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|node| match node {
                        NewMapNode::Land(land) => land,
                        _ => Default::default(),
                    })
                    .collect()
            })
            .collect();

        Ok(())
    };

    let bot = global_bot.clone();
    let update_color = move |payload: String, _| {
        bot.lock().my_color = payload.parse()?;

        Ok(())
    };

    let bot = global_bot.clone();
    let map_update = move |payload: String, socket: RawClient| {
        let [_, map_update]: [_; 2] = serde_json::from_str(&payload)?;

        let mut bot = bot.lock();

        if bot.gm.is_empty() {
            return Ok(());
        }

        if let event::MapUpdate::Data(data) = map_update {
            for [x, y, land] in data {
                bot.gm[x.parse::<usize>()?][y.parse::<usize>()?] = serde_json::from_str(&land)?;
            }
        }

        if config.id > 1 {
            let mut team_won = true;

            'outer: for i in 1..=bot.size {
                for j in 1..=bot.size {
                    let color = bot.gm[i][j].color;

                    if color != 0 && !config.team.contains(bot.color_to_uid.get(&color).unwrap()) {
                        team_won = false;
                        break 'outer;
                    }
                }
            }

            if team_won {
                socket.emit("view", json!(true))?;
                socket.emit("view", json!(false))?;
                return Ok(());
            }
        }

        if let Some(((x1, y1), (x2, y2), half_tag)) = bot.next_move() {
            socket.emit("UploadMovement", json!([x1, y1, x2, y2, half_tag]))?;
        }

        Ok(())
    };

    let bot = global_bot.clone();
    let is_ready = global_is_ready.clone();
    let win_action = move |payload: String, socket| {
        let winner: &str = serde_json::from_str(&payload)?;

        if config.id == 1 && config.bot.team == 0 {
            info!("Room {}: {} won", config.bot.room, winner);
        }

        bot.lock().target = None;

        *is_ready.lock() = false;

        vote_start(&socket, config)
    };

    let bot = global_bot;
    let update_user = move |payload: String, _| {
        let value: serde_json::Value = serde_json::from_str(&payload)?;
        let map = value.as_object().unwrap();

        let mut bot = bot.lock();

        bot.color_to_uid.clear();

        for (uid, value) in map {
            let color = value["color"].as_u64().unwrap() as u8;
            let gaming = value["gaming"].as_bool().unwrap();

            if color != 0 && gaming {
                let uid: u32 = uid.parse()?;

                bot.color_to_uid.insert(color, uid);
            }
        }

        bot.color_to_uid.insert(0, 0);

        Ok(())
    };

    let is_ready = global_is_ready;
    let logged_user_count = move |payload: String, socket: RawClient| {
        if let AutoReady::Conditional { more_than } = config.bot.auto_ready {
            let [count, _]: [u8; 2] = serde_json::from_str(&payload)?;

            let mut is_ready = is_ready.lock();

            if count > more_than && !*is_ready {
                *is_ready = true;
                socket.emit("VoteStart", json!("1"))?;
            } else if count <= more_than && *is_ready {
                *is_ready = false;
                socket.emit("VoteStart", json!("0"))?;
            }
        }

        Ok(())
    };

    ClientBuilder::new(WS_URL)
        .opening_header("cookie", config.bot.cookie)
        .on("open", callback(open))
        .on("close", move |_, _| {
            error!("{} disconnected", config.team[config.id - 1])
        })
        .on("UpdateSettings", callback(update_settings))
        .on("UpdateGM", callback(update_gm))
        .on("UpdateColor", callback(update_color))
        .on("Map_Update", callback(map_update))
        .on("WinAnction", callback(win_action))
        .on("UpdateUser", callback(update_user))
        .on("LoggedUserCount", callback(logged_user_count))
        .connect()?;

    Ok(())
}
