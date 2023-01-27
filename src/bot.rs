use crate::{
    consts::{DIR, EXPAND_SCORE, TARGET_SCORE},
    map::{Land, Map},
    BotData,
};
use fastrand::Rng;
use std::{
    collections::{HashMap, VecDeque},
    ops::Index,
};

pub type Pos = (usize, usize);
pub type Movement = (Pos, Pos, u8);

pub struct Bot {
    pub size: usize,
    pub gm: Map,
    pub my_color: u8,
    pub color_to_uid: HashMap<u8, u32>,
    pub target: Option<Pos>,
    pub from: Option<Pos>,
    config: &'static BotData,
    rng: Rng,
}

impl Index<Pos> for Bot {
    type Output = Land;

    fn index(&self, index: Pos) -> &Self::Output {
        &self.gm[index.0][index.1]
    }
}

impl Bot {
    pub fn new(config: &'static BotData) -> Self {
        Self {
            config,
            rng: Rng::new(),
            size: 0,
            my_color: 0,
            color_to_uid: HashMap::new(),
            target: None,
            from: None,
            gm: Vec::new(),
        }
    }

    #[inline]
    fn is_superior(&self, uid: u32) -> bool {
        matches!(self.config.team.get_index_of(&uid), Some(index) if index + 1 > self.config.id)
    }

    #[inline]
    fn is_valid_pos(&self, (x, y): Pos) -> bool {
        (1..=self.size).contains(&x) && (1..=self.size).contains(&y)
    }

    #[inline]
    fn get_neighbours(&self, (x, y): Pos) -> Vec<Pos> {
        DIR.iter()
            .map(|(dx, dy)| ((x as i8 + dx) as usize, (y as i8 + dy) as usize))
            .filter(|&pos| self.is_valid_pos(pos) && !matches!(self[pos].r#type, 4 | 6))
            .collect()
    }

    #[inline]
    fn iter(&self) -> impl IntoIterator<Item = (Pos, &Land)> + '_ {
        (1..=self.size)
            .flat_map(|x| (1..=self.size).map(move |y| (x, y)))
            .map(|pos| (pos, &self[pos]))
    }

    fn move_to(&self, from: Pos, to: Pos) -> Movement {
        let from_land = &self[from];
        let to_land = &self[to];

        let mut half_tag = 0;

        if !matches!(to_land.r#type, 0 | 5)
            && to_land.color != self.my_color
            && (from_land.amount as i32 - 1) / 2 > to_land.amount as i32
        {
            for neighbour in self.get_neighbours(from) {
                let land = &self[neighbour];

                if land.color != self.my_color && matches!(land.r#type, 2 | 3) && neighbour != to {
                    half_tag = 1;
                    break;
                }
            }
        }

        if to_land.r#type == 5 && from_land.amount > 25 && half_tag == 0 {
            for neighbour in self.get_neighbours(from) {
                let land = &self[neighbour];

                if land.color != self.my_color
                    && matches!(land.r#type, 2 | 3 | 5)
                    && neighbour != to
                {
                    half_tag = 1;
                    break;
                }
            }
        }

        (from, to, half_tag)
    }

    fn visible(&self, (x, y): Pos) -> bool {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let pos = ((x as i8 + dx) as usize, (y as i8 + dy) as usize);

                if self.is_valid_pos(pos) && self[pos].color == self.my_color {
                    return true;
                }
            }
        }

        false
    }

    fn get_new_target(&self) -> Option<Pos> {
        let mut targets = Vec::new();

        for (pos, land) in self.iter() {
            if !matches!(land.r#type, 4 | 6) && land.color != self.my_color && self.visible(pos) {
                let owner_uid = self.color_to_uid.get(&land.color)?;

                if self.is_superior(*owner_uid) {
                    continue;
                }

                targets.push(pos);
            }
        }

        if targets.is_empty() {
            return None;
        }

        self.rng.shuffle(&mut targets);

        let get_score = |&pos: &Pos| {
            let land = &self[pos];
            let mut score = *TARGET_SCORE.get(&land.r#type).unwrap();

            if self
                .config
                .team
                .contains(self.color_to_uid.get(&land.color).unwrap())
            {
                score += 10;
            }

            score
        };

        targets.sort_unstable_by_key(|target| get_score(target));

        targets.first().copied()
    }

    fn move_to_target(&mut self, fallback: bool) -> Option<Movement> {
        if self.target.is_none()
            || matches!(&self.target, Some(target) if self[*target].color == self.my_color)
        {
            self.target = self.get_new_target();
            self.from = None;

            if self.target.is_none() {
                return if fallback { self.expand(false) } else { None };
            }
        }

        let target = self.target.unwrap();

        let get_score = |pos: Pos| {
            let land = &self[pos];

            if land.color == self.my_color {
                land.amount as i32 - 1
            } else {
                -(land.amount as i32) - 1
            }
        };

        let mut max_ans = None;
        let mut max_score = f64::MIN;
        let mut new_from = None;

        let mut q = VecDeque::new();
        let mut vis = HashMap::new();

        let mut found_enemy = false;

        for (pos, land) in self.iter() {
            if land.color != self.my_color && matches!(land.r#type, 1 | 2 | 3) && self.visible(pos)
            {
                found_enemy = true;
                break;
            }
        }

        let mut bfs = |from: Pos| {
            q.clear();
            vis.clear();

            q.push_back((from, get_score(from), 0, None));
            vis.insert(from, true);

            while let Some((cur, amount, length, ans)) = q.pop_front() {
                if cur == target {
                    let score = amount as f64 / (length as f64).powf(1.1);

                    if score > max_score && !(amount < 0 && length < 3) {
                        max_score = score;
                        max_ans = ans;

                        new_from = Some(from);

                        continue;
                    }
                }

                if !found_enemy && length > 6 {
                    continue;
                }

                for nxt in self.get_neighbours(cur) {
                    vis.entry(nxt).or_insert_with(|| {
                        if cur == from {
                            q.push_back((nxt, amount + get_score(nxt), length + 1, Some(nxt)));
                        } else {
                            q.push_back((nxt, amount + get_score(nxt), length + 1, ans));
                        }

                        true
                    });
                }
            }
        };

        let abandon = self.from.is_some() && self.rng.u8(1..=100) >= 70;

        match &self.from {
            Some(from) if !abandon => bfs(*from),
            _ => {
                for (pos, land) in self.iter() {
                    if land.color == self.my_color && land.amount > 1 {
                        let mut flag = true;

                        for neighbour in self.get_neighbours(pos) {
                            let land = &self[neighbour];

                            if land.color != self.my_color && matches!(land.r#type, 2 | 3) {
                                flag = false;
                                break;
                            }
                        }

                        if flag {
                            bfs(pos);
                        }
                    }
                }
            }
        }

        if max_ans.is_none() {
            self.target = None;
            return None;
        }

        if matches!(max_ans, Some(max_ans) if max_ans == target) {
            self.target = None;
        }

        if self.from.is_none() || abandon {
            self.from = new_from;
        }

        let ans = self.move_to(self.from.unwrap(), max_ans.unwrap());
        self.from = max_ans;
        Some(ans)
    }

    fn expand(&mut self, fallback: bool) -> Option<Movement> {
        let mut moves = Vec::new();

        for (from, from_land) in self.iter() {
            if from_land.color == self.my_color {
                for to in self.get_neighbours(from) {
                    let to_land = &self[to];

                    let delta = if to_land.r#type == 3 { 2 } else { 1 };

                    if to_land.color != self.my_color && from_land.amount > to_land.amount + delta {
                        if self.is_superior(*self.color_to_uid.get(&to_land.color)?) {
                            continue;
                        }

                        moves.push((from, to));
                    }
                }
            }
        }

        if moves.is_empty() {
            return if fallback {
                self.move_to_target(false)
            } else {
                None
            };
        }

        self.rng.shuffle(&mut moves);

        let get_score = |&from: &Pos, &to: &Pos| {
            let from_land = &self[from];
            let to_land = &self[to];

            let mut score = EXPAND_SCORE.get(&to_land.r#type).unwrap().to_owned();

            if from_land.r#type == 2 && matches!(to_land.r#type, 1 | 3) {
                score -= 20 - (from_land.amount - to_land.amount).min(10) as i8;
            }

            let (_, _, half_tag) = self.move_to(from, to);

            let from_remain = if half_tag == 1 {
                from_land.amount / 2
            } else {
                1
            };

            for neighbour in self.get_neighbours(from) {
                if self[neighbour].color != self.my_color
                    && self[neighbour].amount > from_remain + 1
                    && neighbour != to
                {
                    score += 10;
                    break;
                }
            }

            let to_remain = from_land.amount - from_remain - to_land.amount;

            for neighbour in self.get_neighbours(to) {
                if self[neighbour].color != self.my_color && self[neighbour].amount > to_remain + 1
                {
                    score += 10;
                    break;
                }
            }

            if self
                .config
                .team
                .contains(self.color_to_uid.get(&to_land.color).unwrap())
            {
                score += 100;
            }

            score
        };

        moves.sort_unstable_by_key(|(from, to)| get_score(from, to));

        let (from, to) = moves.first()?;

        Some(self.move_to(*from, *to))
    }

    pub fn next_move(&mut self) -> Option<Movement> {
        if self.rng.u8(1..=100) > self.config.bot.expand_rate {
            self.move_to_target(true)
        } else {
            self.expand(true)
        }
    }
}
