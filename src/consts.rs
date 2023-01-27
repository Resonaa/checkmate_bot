use std::collections::HashMap;

pub static DIR: [(i8, i8); 4] = [(-1, 0), (0, 1), (1, 0), (0, -1)];
pub static WS_URL: &str = "https://kana.byha.top:444/ws/checkmate/";
pub static HALL_URL: &str = "https://kana.byha.top:444/checkmate/room";

lazy_static! {
    pub static ref TARGET_SCORE: HashMap<u8, i8> =
        HashMap::from([(1, 1), (3, 2), (2, 2), (0, 3), (5, 4)]);
    pub static ref EXPAND_SCORE: HashMap<u8, i8> =
        HashMap::from([(1, 1), (3, 2), (2, 3), (5, 4), (0, 5)]);
}
