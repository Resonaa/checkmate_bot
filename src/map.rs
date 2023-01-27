use serde::Deserialize;

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
