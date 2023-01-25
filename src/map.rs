#![allow(clippy::too_many_arguments)]

use crate::BotData;
use serde::Deserialize;
use std::collections::{HashMap, VecDeque};

#[derive(Deserialize, Default)]
pub struct Land {
    pub color: u8,
    pub r#type: u8,
    pub amount: u32,
}

#[derive(Deserialize)]
pub struct MapInfo {
    pub size: usize,
    pub r#type: u8,
}

pub type Map = Vec<Vec<Land>>;
pub type Pos = (usize, usize);
pub type Movement = (Pos, Pos, u8);

static DIR: [[i8; 2]; 4] = [[-1, 0], [0, 1], [1, 0], [0, -1]];

fn is_superior(uid: &u32, config: &BotData) -> bool {
    matches!(config.team.get_index_of(uid), Some(index) if index + 1 > config.id)
}

fn get_neighbours(gm: &Map, size: usize, pos: Pos) -> Vec<Pos> {
    let mut ans = Vec::new();

    for [dx, dy] in DIR {
        let px = (pos.0 as i8 + dx) as usize;
        let py = (pos.1 as i8 + dy) as usize;

        if (1..=size).contains(&px)
            && (1..=size).contains(&py)
            && !matches!(gm[px][py].r#type, 4 | 6)
        {
            ans.push((px, py));
        }
    }

    ans
}

fn move_to(gm: &Map, size: usize, my_color: u8, sp: Pos, ep: Pos) -> Movement {
    let s = &gm[sp.0][sp.1];
    let e = &gm[ep.0][ep.1];

    let mut half_tag = 0;

    if !matches!(e.r#type, 0 | 5)
        && e.color != my_color
        && (s.amount as i32 - 1) / 2 > e.amount as i32
    {
        let mut flag = 0;

        for (nx, ny) in get_neighbours(gm, size, sp) {
            let node = &gm[nx][ny];

            if node.color != my_color && matches!(node.r#type, 2 | 3) && (nx != ep.0 || ny != ep.1)
            {
                flag = 1;
                break;
            }
        }

        half_tag = flag;
    }

    if e.r#type == 5 && s.amount > 25 && half_tag == 0 {
        let mut flag = 0;

        for (nx, ny) in get_neighbours(gm, size, sp) {
            let node = &gm[nx][ny];

            if node.color != my_color
                && matches!(node.r#type, 2 | 3 | 5)
                && (nx != ep.0 || ny != ep.1)
            {
                flag = 1;
                break;
            }
        }

        half_tag = flag;
    }

    (sp, ep, half_tag)
}

fn visible(gm: &Map, size: usize, my_color: u8, (x, y): Pos) -> bool {
    for dx in -1..=1 {
        for dy in -1..=1 {
            let px = (x as i8 + dx) as usize;
            let py = (y as i8 + dy) as usize;

            if (1..=size).contains(&px) && (1..=size).contains(&py) && gm[px][py].color == my_color
            {
                return true;
            }
        }
    }

    false
}

#[allow(clippy::needless_range_loop)]
fn change_target(
    gm: &Map,
    size: usize,
    my_color: u8,
    color_to_uid: &HashMap<u8, u32>,
    config: &BotData,
) -> Option<Pos> {
    let score: HashMap<u8, u8> = HashMap::from([(1, 1), (3, 2), (2, 2), (0, 3), (5, 4)]);

    let mut tmp = Vec::new();

    for i in 1..=size {
        for j in 1..=size {
            let node = &gm[i][j];

            if !matches!(node.r#type, 4 | 6)
                && node.color != my_color
                && visible(gm, size, my_color, (i, j))
            {
                let uid = color_to_uid.get(&node.color)?;

                if is_superior(uid, config) {
                    continue;
                }

                tmp.push((i, j));
            }
        }
    }

    if tmp.is_empty() {
        return None;
    }

    fastrand::shuffle(&mut tmp);

    let get_score = |&(x, y): &Pos| {
        let land = &gm[x][y];
        let mut score = score.get(&land.r#type).unwrap().to_owned();

        if config.team.contains(color_to_uid.get(&land.color).unwrap()) {
            score += 10;
        }

        score
    };

    tmp.sort_unstable_by_key(|a| get_score(a));

    tmp.first().copied()
}

fn next_move(
    gm: &Map,
    size: usize,
    my_color: u8,
    color_to_uid: &HashMap<u8, u32>,
    config: &BotData,
    target: &mut Option<Pos>,
    from: &mut Option<Pos>,
    no_recursion: bool,
) -> Option<Movement> {
    if target.is_none() || matches!(&target, Some((x, y)) if gm[*x][*y].color == my_color) {
        *target = change_target(gm, size, my_color, color_to_uid, config);
        *from = None;

        if target.is_none() {
            return if no_recursion {
                None
            } else {
                expand(gm, size, my_color, color_to_uid, config, target, from, true)
            };
        }
    }

    let (target_x, target_y) = target.unwrap();

    let get_score = |node: &Land| {
        if node.color == my_color {
            node.amount as i32 - 1
        } else {
            -(node.amount as i32) - 1
        }
    };

    let mut max_ans = None;
    let mut max_score = f64::MIN;
    let mut new_from = None;

    let mut q = VecDeque::new();
    let mut vis = HashMap::new();

    let found_enemy = {
        let mut ans = false;

        'outer: for i in 1..=size {
            for j in 1..=size {
                if gm[i][j].color != my_color
                    && matches!(gm[i][j].r#type, 1 | 2 | 3)
                    && visible(gm, size, my_color, (i, j))
                {
                    ans = true;
                    break 'outer;
                }
            }
        }

        ans
    };

    let mut bfs = |i, j| {
        q.clear();
        vis.clear();

        let row: &Vec<Land> = &gm[i];
        let land: &Land = &row[j];

        q.push_back(((i, j), land.amount as i32, 0, None));
        vis.insert((i, j), true);

        while let Some(((cur_x, cur_y), amount, length, ans)) = q.pop_front() {
            if cur_x == target_x && cur_y == target_y {
                let score = amount as f64 / (length as f64).powf(1.1);

                if score > max_score && amount > 0 {
                    max_score = score;
                    max_ans = ans;

                    new_from = Some((i, j));

                    continue;
                }
            }

            if !found_enemy && length > 6 {
                continue;
            }

            for nxt in get_neighbours(gm, size, (cur_x, cur_y)) {
                vis.entry(nxt).or_insert_with(|| {
                    if cur_x == i && cur_y == j {
                        q.push_back((
                            nxt,
                            amount + get_score(&gm[nxt.0][nxt.1]),
                            length + 1,
                            Some(nxt),
                        ));
                    } else {
                        q.push_back((nxt, amount + get_score(&gm[nxt.0][nxt.1]), length + 1, ans));
                    }

                    true
                });
            }
        }
    };

    let drop = from.is_some() && fastrand::u8(1..=100) >= 70;

    match &from {
        Some((x, y)) if !drop => bfs(*x, *y),
        _ => {
            for i in 1..=size {
                for j in 1..=size {
                    if gm[i][j].color == my_color && gm[i][j].amount > 1 {
                        let mut flag = true;

                        for (nx, ny) in get_neighbours(gm, size, (i, j)) {
                            let node = &gm[nx][ny];

                            if node.color != my_color && matches!(node.r#type, 2 | 3) {
                                flag = false;
                                break;
                            }
                        }

                        if flag {
                            bfs(i, j);
                        }
                    }
                }
            }
        }
    }

    if max_ans.is_none() {
        *target = None;
        return None;
    }

    if matches!(max_ans, Some((x, y)) if x == target_x && y == target_y) {
        *target = None;
    }

    if from.is_none() || drop {
        *from = new_from;
    }

    let ans = move_to(gm, size, my_color, from.unwrap(), max_ans.unwrap());
    *from = max_ans;
    Some(ans)
}

fn expand(
    gm: &Map,
    size: usize,
    my_color: u8,
    color_to_uid: &HashMap<u8, u32>,
    config: &BotData,
    target: &mut Option<Pos>,
    from: &mut Option<Pos>,
    no_recursion: bool,
) -> Option<Movement> {
    let score: HashMap<u8, u8> = HashMap::from([(1, 1), (3, 2), (2, 3), (5, 4), (0, 5)]);

    let mut tmp = Vec::new();

    for i in 1..=size {
        for j in 1..=size {
            if gm[i][j].color == my_color {
                let start = (i, j);
                let start_node = &gm[i][j];

                for end in get_neighbours(gm, size, start) {
                    let end_node = &gm[end.0][end.1];

                    let delta = if end_node.r#type == 3 { 2 } else { 1 };

                    if end_node.color != my_color && start_node.amount > end_node.amount + delta {
                        if is_superior(color_to_uid.get(&end_node.color)?, config) {
                            continue;
                        }

                        tmp.push((start, end));
                    }
                }
            }
        }
    }

    if tmp.is_empty() {
        return if no_recursion {
            None
        } else {
            next_move(gm, size, my_color, color_to_uid, config, target, from, true)
        };
    }

    fastrand::shuffle(&mut tmp);

    let get_score = |from: &Pos, to: &Pos| {
        let land = &gm[from.0][from.1];
        let mut score = score.get(&land.r#type).unwrap().to_owned();

        if config.team.contains(color_to_uid.get(&land.color).unwrap()) {
            score += 50;
        }

        let (_, _, half_tag) = move_to(gm, size, my_color, *from, *to);

        let remain = if half_tag == 1 { land.amount / 2 } else { 1 };

        let mut flag = false;

        for (nx, ny) in get_neighbours(gm, size, *from) {
            if gm[nx][ny].color != my_color
                && gm[nx][ny].amount > remain + 1
                && (nx != to.0 || ny != to.1)
            {
                flag = true;
                break;
            }
        }

        if flag {
            score += 10;
        }

        let to_remain = land.amount - remain - gm[to.0][to.1].amount;

        let mut flag = false;

        for (nx, ny) in get_neighbours(gm, size, *to) {
            if gm[nx][ny].color != my_color && gm[nx][ny].amount > to_remain + 1 {
                flag = true;
                break;
            }
        }

        if flag {
            score += 10;
        }

        score
    };

    tmp.sort_unstable_by_key(|(from, to)| get_score(from, to));

    let (start, end) = tmp.first().copied()?;

    Some(move_to(gm, size, my_color, start, end))
}

pub fn bot_move(
    gm: &Map,
    size: usize,
    my_color: u8,
    color_to_uid: &HashMap<u8, u32>,
    config: &BotData,
    target: &mut Option<Pos>,
    from: &mut Option<Pos>,
) -> Option<Movement> {
    if fastrand::u8(1..=100) > config.bot.expand_rate {
        next_move(
            gm,
            size,
            my_color,
            color_to_uid,
            config,
            target,
            from,
            false,
        )
    } else {
        expand(
            gm,
            size,
            my_color,
            color_to_uid,
            config,
            target,
            from,
            false,
        )
    }
}
