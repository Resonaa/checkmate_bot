pub static DIR: [(i8, i8); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];
pub static WS_URL: &str = "https://kana.byha.top:444/ws/checkmate/";
pub static HALL_URL: &str = "https://kana.byha.top:444/checkmate/room";
pub static TARGET_SCORE: [i8; 6] = [2, 1, 1, 1, 9, 3];
pub static EXPAND_SCORE: [i8; 6] = [5, 1, 3, 2, 9, 4];

pub const SCORE_POWER: f64 = 1.0;

pub const fn default_calc_cnt() -> u8 {
    1
}
