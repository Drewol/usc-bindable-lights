use std::{path::PathBuf, str::FromStr};

use crate::lighting::LightingMap;

pub type Config = LightingMap;

pub fn path() -> PathBuf {
    let Some(mut dir) = dirs::config_dir() else {
        return PathBuf::from_str("usc-lights.toml").unwrap();
    };

    dir.push("usc-lights");
    dir.push("config.toml");
    dir
}

pub fn load() -> Config {
    std::fs::read_to_string(path())
        .map(|d| toml::from_str::<Config>(&d).unwrap_or_default())
        .unwrap_or_default()
}
