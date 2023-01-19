use crate::{
    event::{self, callback},
    map, AutoReady, BotData,
};
use anyhow::Result;
use parking_lot::Mutex;
use rust_socketio::{ClientBuilder, RawClient};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};

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
    let global_vote = Arc::new(Mutex::new(false));
    let global_size = Arc::new(Mutex::new(0));
    let global_gm = Arc::new(Mutex::new(Vec::<Vec<_>>::new()));
    let global_my_color = Arc::new(Mutex::new(0));
    let global_color_to_uid = Arc::new(Mutex::new(HashMap::new()));
    let global_target = Arc::new(Mutex::new(None));
    let global_from = Arc::new(Mutex::new(None));

    let open = move |_, socket: RawClient| {
        info!("{} connected", config.team[config.id - 1]);
        socket.emit("joinRoom", config.bot.room)?;

        vote_start(&socket, config)?;

        Ok(())
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

    let gm = global_gm.clone();
    let size = global_size.clone();
    let my_color = global_my_color.clone();
    let update_gm = move |payload: String, _| {
        use event::NewMapNode;

        let update_gm: Vec<Vec<_>> = serde_json::from_str(&payload)?;

        if let NewMapNode::MapInfo(map_info) = &update_gm[0][0] {
            *size.lock() = map_info.size;
        }

        *gm.lock() = update_gm
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|node| match node {
                        NewMapNode::MapInfo(_) => map::Land {
                            ..Default::default()
                        },
                        NewMapNode::Land(land) => land,
                    })
                    .collect()
            })
            .collect();

        Ok(())
    };

    let update_color = move |payload: String, _| {
        *my_color.lock() = payload.parse()?;

        Ok(())
    };

    let gm = global_gm;
    let size = global_size;
    let my_color = global_my_color;
    let color_to_uid = global_color_to_uid.clone();
    let target = global_target.clone();
    let from = global_from;
    let map_update = move |payload: String, socket: RawClient| {
        let map_update: [event::MapUpdate; 2] = serde_json::from_str(&payload)?;

        let mut gm = gm.lock();

        if gm.len() == 0 {
            return Ok(());
        }

        if let event::MapUpdate::Data(data) = &map_update[1] {
            for [x, y, land] in data {
                gm[x.parse::<usize>()?][y.parse::<usize>()?] = serde_json::from_str(land)?;
            }
        }

        let size = size.lock();
        let my_color = my_color.lock();
        let color_to_uid = color_to_uid.lock();
        let mut target = target.lock();
        let mut from = from.lock();

        if config.id > 1 {
            let mut flag = true;

            'outer: for i in 1..=*size {
                for j in 1..=*size {
                    let color = gm[i][j].color;

                    if color != 0 && !config.team.contains(color_to_uid.get(&color).unwrap()) {
                        flag = false;
                        break 'outer;
                    }
                }
            }

            if flag {
                socket.emit("view", json!(true))?;
                socket.emit("view", json!(false))?;
                return Ok(());
            }
        }

        let movememt = map::bot_move(
            &gm,
            *size,
            *my_color,
            &color_to_uid,
            config,
            &mut target,
            &mut from,
        );

        if let Some(((x1, y1), (x2, y2), half_tag)) = movememt {
            socket.emit("UploadMovement", json!([x1, y1, x2, y2, half_tag]))?;
        }

        Ok(())
    };

    let target = global_target;
    let vote = global_vote.clone();
    let win_action = move |payload: String, socket| {
        let winner: String = serde_json::from_str(&payload)?;

        if config.id == 1 && config.bot.team == 0 {
            info!("Room {}: {} won", config.bot.room, winner);
        }

        *target.lock() = None;

        vote_start(&socket, config)?;

        *vote.lock() = false;

        Ok(())
    };

    let color_to_uid = global_color_to_uid;
    let update_user = move |payload: String, _| {
        let value: serde_json::Value = serde_json::from_str(&payload)?;
        let map = value.as_object().unwrap();

        let mut color_to_uid = color_to_uid.lock();
        color_to_uid.clear();

        for (uid, value) in map {
            let color = value["color"].as_u64().unwrap() as u8;
            let gaming = value["gaming"].as_bool().unwrap();

            if color != 0 && gaming {
                let uid: u32 = uid.parse()?;

                color_to_uid.insert(color, uid);
            }
        }

        color_to_uid.insert(0, 0);

        Ok(())
    };

    let vote = global_vote;
    let logged_user_count = move |payload: String, socket: RawClient| {
        if let AutoReady::Conditional { more_than } = config.bot.auto_ready {
            let [count, _]: [u8; 2] = serde_json::from_str(&payload)?;

            let mut vote = vote.lock();

            if count > more_than && !*vote {
                *vote = true;
                socket.emit("VoteStart", json!("1"))?;
            } else if count <= more_than && *vote {
                *vote = false;
                socket.emit("VoteStart", json!("0"))?;
            }
        }

        Ok(())
    };

    ClientBuilder::new("https://kana.byha.top:444/ws/checkmate/")
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
