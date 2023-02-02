use crate::{
    consts::{DIR, EXPAND_SCORE, SCORE_POWER, TARGET_SCORE},
    map::{Land, Map},
    BotData,
};
use fastrand::Rng;
use std::{
    collections::{HashMap, VecDeque},
    ops::Index,
};

type Pos = (usize, usize);
type Movement = (Pos, Pos, u8);

pub struct Bot {
    pub size: usize,
    pub gm: Map,
    pub my_color: u8,
    pub color_to_uid: HashMap<u8, u32>,
    pub target: Option<Pos>,
    from: Option<Pos>,
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
    fn superior(&self, uid: u32) -> bool {
        matches!(self.config.team.get_index_of(&uid), Some(index) if index + 1 > self.config.id)
    }

    #[inline]
    const fn valid_pos(&self, (x, y): Pos) -> bool {
        x >= 1 && x <= self.size && y >= 1 && y <= self.size
    }

    #[inline]
    fn neighbours(&self, (x, y): Pos) -> Vec<Pos> {
        DIR.iter()
            .map(|(dx, dy)| ((x as i8 + dx) as usize, (y as i8 + dy) as usize))
            .filter(|&pos| self.valid_pos(pos) && !matches!(self[pos].r#type, 4 | 6))
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
            for neighbour in self.neighbours(from) {
                let land = &self[neighbour];

                if land.color != self.my_color && matches!(land.r#type, 2 | 3) && neighbour != to {
                    half_tag = 1;
                    break;
                }
            }
        }

        if to_land.r#type == 5 && from_land.amount > 25 && half_tag == 0 {
            for neighbour in self.neighbours(from) {
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

                if self.valid_pos(pos) && self[pos].color == self.my_color {
                    return true;
                }
            }
        }

        false
    }

    fn new_target(&self) -> Option<Pos> {
        let mut targets = Vec::new();

        for (pos, land) in self.iter() {
            if !matches!(land.r#type, 4 | 6) && land.color != self.my_color && self.visible(pos) {
                let owner_uid = self.color_to_uid.get(&land.color)?;

                if self.superior(*owner_uid) {
                    continue;
                }

                targets.push(pos);
            }
        }

        self.rng.shuffle(&mut targets);

        let get_score = |&pos: &Pos| {
            let land = &self[pos];
            let mut score = TARGET_SCORE[land.r#type as usize];

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

    pub fn expand(&mut self) -> Option<Movement> {
        let mut moves = Vec::new();

        for (from, from_land) in self.iter() {
            if from_land.color == self.my_color {
                for to in self.neighbours(from) {
                    let to_land = &self[to];

                    let delta = if to_land.r#type == 3 { 2 } else { 1 };

                    if to_land.color != self.my_color && from_land.amount > to_land.amount + delta {
                        if self.superior(*self.color_to_uid.get(&to_land.color)?) {
                            continue;
                        }

                        moves.push((from, to));
                    }
                }
            }
        }

        self.rng.shuffle(&mut moves);

        let get_score = |&from: &Pos, &to: &Pos| {
            let from_land = &self[from];
            let to_land = &self[to];

            let mut score = EXPAND_SCORE[to_land.r#type as usize];

            if from_land.r#type == 2 && matches!(to_land.r#type, 1 | 3) {
                score -= 20 - (from_land.amount - to_land.amount).min(10) as i8;
            }

            let (_, _, half_tag) = self.move_to(from, to);

            let from_remain = if half_tag == 1 {
                from_land.amount / 2
            } else {
                1
            };

            for neighbour in self.neighbours(from) {
                if self[neighbour].color != self.my_color
                    && self[neighbour].amount > from_remain + 1
                    && neighbour != to
                {
                    score += 10;
                    break;
                }
            }

            let to_remain = from_land.amount - from_remain - to_land.amount;

            for neighbour in self.neighbours(to) {
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

        match moves.first() {
            Some(&(from, to)) => {
                if Some(from) == self.from && Some(to) != self.target {
                    self.target = None;
                }

                Some(self.move_to(from, to))
            }
            None => self.move_to_target(0),
        }
    }

    fn move_to_target(&mut self, try_time: u8) -> Option<Movement> {
        if try_time >= self.config.bot.calc_cnt {
            return None;
        }

        if self.target.is_none()
            || matches!(&self.target, Some(target) if self[*target].color == self.my_color)
        {
            self.target = self.new_target();
            self.from = None;
        }

        let target = self.target?;

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
            let mut tmp_ans = None;
            let mut tmp_score = f64::MIN;
            let mut tmp_from = None;

            for try_time in 0..self.config.bot.calc_cnt {
                q.clear();
                vis.clear();

                q.push_back((from, get_score(from), 0, None));
                vis.insert(from, ());

                while let Some((cur, amount, length, ans)) = q.pop_front() {
                    if cur == target {
                        let score = amount as f64 / (length as f64).powf(SCORE_POWER);

                        if score > tmp_score && !(amount < 0 && length < 2) {
                            tmp_score = score;
                            tmp_ans = ans;

                            tmp_from = Some(from);

                            continue;
                        }
                    }

                    if !found_enemy && length > 6 {
                        continue;
                    }

                    let mut neighbours = self.neighbours(cur);
                    self.rng.shuffle(&mut neighbours);

                    for nxt in neighbours {
                        vis.entry(nxt).or_insert_with(|| {
                            if cur == from {
                                q.push_back((nxt, amount + get_score(nxt), length + 1, Some(nxt)));
                            } else {
                                q.push_back((nxt, amount + get_score(nxt), length + 1, ans));
                            }
                        });
                    }
                }

                if try_time == 2 && tmp_score < max_score / 2.0 {
                    break;
                }
            }

            if tmp_score > max_score {
                max_score = tmp_score;
                max_ans = tmp_ans;
                new_from = tmp_from;
            }
        };

        match &self.from {
            Some(from) => bfs(*from),
            _ => {
                'outer: for (pos, land) in self.iter() {
                    if land.color == self.my_color && land.amount > 1 {
                        for neighbour in self.neighbours(pos) {
                            let land = &self[neighbour];

                            if land.color != self.my_color && matches!(land.r#type, 0 | 2 | 3) {
                                continue 'outer;
                            }
                        }

                        bfs(pos);
                    }
                }
            }
        }

        if max_ans.is_none() {
            self.target = None;
            return self.move_to_target(try_time + 1);
        }

        let max_ans = max_ans.unwrap();

        if max_ans == target {
            self.target = None;
        }

        if self.from.is_none() {
            self.from = new_from;
        }

        let ans = self.move_to(self.from.unwrap(), max_ans);
        self.from = Some(max_ans);
        Some(ans)
    }
}
