use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProfileApp {
    pub desktop: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silent: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Config {
    pub active_profile: String,
    pub profiles: HashMap<String, Vec<ProfileApp>>,
}

impl Default for Config {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(
            "default".to_string(),
            vec![
                ProfileApp {
                    desktop: "firefox.desktop".to_string(),
                    workspace: Some("1".to_string()),
                    silent: Some(false),
                },
                ProfileApp {
                    desktop: "kitty.desktop".to_string(),
                    workspace: Some("2".to_string()),
                    silent: Some(true),
                },
            ],
        );
        Config {
            active_profile: "default".to_string(),
            profiles,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    if let Ok(path_str) = std::env::var("HYPRLAUNCH_CONFIG") {
        PathBuf::from(path_str)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        Path::new(&home).join(".config/hyprlaunch/config.json")
    }
}

pub fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path = get_config_path();
    if !path.exists() {
        let config = Config::default();
        save_config(&config)?;
        return Ok(config);
    }

    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    let json = serde_json::to_string_pretty(config)?;
    file.write_all(json.as_bytes())?;
    Ok(())
}
