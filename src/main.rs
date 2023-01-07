use anyhow::Result;
use checkmate_bot::{bot::new_bot, Role};
use std::{thread, time::Duration};

fn main() -> Result<()> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    new_bot(Role::Upgrader)?;
    new_bot(Role::Player)?;

    loop {
        thread::sleep(Duration::from_secs(10000));
    }
}
