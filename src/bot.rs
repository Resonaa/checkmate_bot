use crate::{
    event::{self, callback},
    map, Role, CONFIG,
};
use anyhow::Result;
use chrono::prelude::*;
use hashbrown::HashMap;
use parking_lot::Mutex;
use regex::Regex;
use rust_socketio::{ClientBuilder, RawClient};
use serde::Deserialize;
use serde_json::json;
use std::{fs, sync::Arc};

fn vote_map(socket: &RawClient) -> Result<()> {
    socket.emit("changeSettings", json!({"map": CONFIG.map.to_string()}))?;

    Ok(())
}

fn send_message(socket: &RawClient, message: &str) -> Result<()> {
    socket.emit("SendWorldMessage", json!(message))?;

    Ok(())
}

pub fn new_bot(role: Role) -> Result<()> {
    let cookie = match role {
        Role::Upgrader => CONFIG.cookie_1.clone(),
        Role::Player => CONFIG.cookie_2.clone(),
    };

    let global_vote = Arc::new(Mutex::new(false));
    let global_challenger = Arc::new(Mutex::new(0));
    let global_round = Arc::new(Mutex::new(0));
    let global_size = Arc::new(Mutex::new(0));
    let global_gm: Arc<Mutex<Vec<Vec<map::Land>>>> = Arc::new(Mutex::new(Vec::new()));
    let global_my_color = Arc::new(Mutex::new(0));
    let global_color_to_uid = Arc::new(Mutex::new(HashMap::new()));
    let global_target = Arc::new(Mutex::new(None));
    let global_from = Arc::new(Mutex::new(None));

    let open = move |_, socket: RawClient| {
        info!("{:?} connected", role);
        socket.emit("joinRoom", CONFIG.room.clone())?;

        vote_map(&socket)?;

        if role == Role::Upgrader {
            socket.emit("VoteStart", json!(1))?;
        }

        Ok(())
    };

    let update_settings = |payload: String, socket: RawClient| {
        let update_settings: event::UpdateSettings = serde_json::from_str(&payload)?;

        match update_settings.speed {
            event::Speed::U8(speed) => {
                if speed != CONFIG.speed {
                    socket.emit("changeSettings", json!({"speed": CONFIG.speed}))?;
                }
            }
            event::Speed::String(speed) => {
                if speed != CONFIG.speed.to_string() {
                    socket.emit("changeSettings", json!({"speed": CONFIG.speed}))?;
                }
            }
        }

        if update_settings.private != CONFIG.private {
            socket.emit("changeSettings", json!({"private": CONFIG.private}))?;
        }

        Ok(())
    };

    let gm = global_gm.clone();
    let size = global_size.clone();
    let my_color = global_my_color.clone();
    let challenger = global_challenger.clone();
    let update_gm = move |payload: String, _| {
        let update_gm: Vec<Vec<event::NewMapNode>> = serde_json::from_str(&payload)?;

        use event::NewMapNode;

        if let NewMapNode::MapInfo(map_info) = &update_gm[0][0] {
            *size.lock() = map_info.size;
        }

        let mut gm = gm.lock();

        *gm = update_gm
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

        let player_count = gm.iter().flatten().filter(|land| land.r#type == 1).count();

        if player_count == 3 {
            *challenger.lock() = -1;
        }

        Ok(())
    };

    let update_color = move |payload: String, _| {
        *my_color.lock() = payload.parse()?;

        Ok(())
    };

    let gm = global_gm.clone();
    let size = global_size.clone();
    let my_color = global_my_color;
    let color_to_uid = global_color_to_uid.clone();
    let target = global_target.clone();
    let from = global_from;
    let round = global_round.clone();
    let challenger = global_challenger.clone();
    let map_update = move |payload: String, socket: RawClient| {
        let map_update: [event::MapUpdate; 2] = serde_json::from_str(&payload)?;

        let challenger = challenger.lock();
        let mut round = round.lock();

        if let event::MapUpdate::Round(new_round) = &map_update[0] {
            *round = *new_round;

            if new_round % 100 == 0
                && role == Role::Upgrader
                && *challenger > 0
                && *new_round < 1000
            {
                send_message(&socket, &format!("{}回合", new_round))?;
            }
        }

        let mut gm = gm.lock();

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

        if role == Role::Player {
            let mut flag = true;

            'outer: for i in 1..=*size {
                for j in 1..=*size {
                    let color = gm[i][j].color;

                    if color != 0 && !CONFIG.bot_uid.contains(color_to_uid.get(&color).unwrap()) {
                        flag = false;
                        break 'outer;
                    }
                }
            }

            if flag || *challenger > 0 && *round >= 1000 {
                socket.emit("view", json!(true))?;
                socket.emit("view", json!(false))?;
                return Ok(());
            }
        }

        let movememt = if *challenger > 0 {
            None
        } else {
            map::bot_move(
                &gm,
                *size,
                *my_color,
                &color_to_uid,
                role,
                &mut target,
                &mut from,
            )
        };

        if let Some(((x1, y1), (x2, y2), half_tag)) = movememt {
            socket.emit("UploadMovement", json!([x1, y1, x2, y2, half_tag]))?;
        }

        Ok(())
    };

    let target = global_target;
    let challenger = global_challenger.clone();
    let gm = global_gm;
    let color_to_uid = global_color_to_uid.clone();
    let round = global_round;
    let size = global_size;
    let win_action = move |payload: String, socket| {
        let winner: String = serde_json::from_str(&payload)?;

        if role == Role::Upgrader {
            info!("{}赢了", winner);
        }

        *target.lock() = None;

        vote_map(&socket)?;

        if role == Role::Upgrader {
            socket.emit("VoteStart", json!(1))?;

            let mut challenger = challenger.lock();

            if *challenger <= 0 {
                return Ok(());
            }

            #[derive(Deserialize)]
            struct Name2IdResponse {
                pub msg: i32,
            }

            let client = reqwest::blocking::Client::new();

            if let Ok(uid) = client
                .get(format!(
                    "https://kana.byha.top:444/api/user/name2id?uname={}",
                    winner
                ))
                .header("cookie", &CONFIG.cookie_1)
                .send()?
                .json::<Name2IdResponse>()
            {
                let uid = uid.msg;
                let round = round.lock();

                if uid != *challenger {
                    send_message(&socket, "挑战失败")?;
                } else if *round >= 1000 {
                    send_message(&socket, "挑战超时")?;
                } else {
                    let challenger_color = color_to_uid
                        .lock()
                        .iter()
                        .find(|(_, &_uid)| _uid as i32 == uid)
                        .unwrap()
                        .0
                        .to_owned();

                    let mut remains = 0;

                    let size = size.lock();
                    let gm = gm.lock();

                    for i in 1..=*size {
                        for j in 1..=*size {
                            if gm[i][j].color != challenger_color
                                && !matches!(gm[i][j].r#type, 4 | 6)
                            {
                                remains += 1;
                            }
                        }
                    }

                    if remains > 0 {
                        send_message(&socket, &format!("挑战失败: 剩余{}格", remains))?;
                    } else {
                        send_message(
                            &socket,
                            &format!(
                                "{}挑战成功: {}回合<br><a href=\"/post/{}\">排行榜</a>",
                                winner, round, CONFIG.leaderboard_id
                            ),
                        )?;

                        let replay_data = client
                            .get("https://kana.byha.top:444/admin/battle?page=1")
                            .header("cookie", &CONFIG.cookie_1)
                            .send()?
                            .text()?;

                        let re = Regex::new("battle_id=\"(\\w*)\"")?;

                        let rid = re
                            .captures(&replay_data)
                            .and_then(|caps| caps.get(1))
                            .unwrap()
                            .as_str();

                        let data = match fs::read_to_string("data.json") {
                            Ok(data) => data,
                            Err(_) => "{}".to_string(),
                        };

                        let mut data: std::collections::HashMap<i32, (u32, &str)> =
                            serde_json::from_str(&data)?;

                        match data.get(&uid) {
                            Some(score) => {
                                if *round < score.0 {
                                    data.insert(uid, (*round, rid));
                                }
                            }
                            None => {
                                data.insert(uid, (*round, rid));
                            }
                        }

                        fs::write("data.json", serde_json::to_string(&data)?)?;

                        let mut ans = "# 分数排行榜\n\n".to_string();

                        ans +=
                            &format!("更新时间: {}\n\n", Local::now().format("%Y-%m-%d %H:%M:%S"));

                        let mut data: Vec<_> = data.into_iter().collect();

                        data.sort_unstable_by(|a, b| {
                            if a.1 .0 != b.1 .0 {
                                a.1 .0.cmp(&b.1 .0)
                            } else {
                                a.0.cmp(&b.0)
                            }
                        });

                        for (rank, (uid, (round, rid))) in data.into_iter().enumerate() {
                            ans += &format!("{}. [at,uid={uid}], {round}回合, [回放](/checkmate/replay/{rid})\n", rank + 1);
                        }

                        client
                            .post("https://kana.byha.top:444/api/updatepost")
                            .header("cookie", &CONFIG.cookie_1)
                            .json(&json!({"pid": CONFIG.leaderboard_id, "content": ans}))
                            .send()?;
                    }
                }
            }

            *challenger = 0;
        }

        Ok(())
    };

    let color_to_uid = global_color_to_uid;
    let challenger = global_challenger;
    let update_user = move |payload: String, socket: RawClient| {
        let value: serde_json::Value = serde_json::from_str(&payload)?;
        let map = value.as_object().unwrap();

        let mut color_to_uid = color_to_uid.lock();
        color_to_uid.clear();

        let mut challenger = challenger.lock();

        for (uid, value) in map {
            let color = value["color"].as_u64().unwrap() as u8;

            if color != 0 {
                let uid = uid.parse::<u32>()?;

                if !CONFIG.bot_uid.contains(&uid) && *challenger == -1 {
                    *challenger = uid as i32;

                    if role == Role::Upgrader {
                        send_message(&socket, "挑战开始")?;
                    }
                }

                color_to_uid.insert(color, uid);
            }
        }

        color_to_uid.insert(0, 0);

        Ok(())
    };

    let vote = global_vote;
    let logged_user_count = move |payload: String, socket: RawClient| {
        if role == Role::Player {
            let [count, _]: [u8; 2] = serde_json::from_str(&payload)?;

            let mut vote = vote.lock();

            if count >= 4 && !*vote {
                *vote = true;
                socket.emit("VoteStart", json!("1"))?;
            } else if count < 4 && *vote {
                *vote = false;
                socket.emit("VoteStart", json!("0"))?;
            }
        }

        Ok(())
    };

    ClientBuilder::new("https://kana.byha.top:444/ws/checkmate/")
        .opening_header("cookie", cookie)
        .on("open", callback(open))
        .on("close", move |_, _| error!("{:?} disconnected", role))
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
