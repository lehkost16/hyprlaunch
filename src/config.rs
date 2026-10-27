use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicy {
    #[default]
    Continue,
    Stop,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StepCondition {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub if_battery: Option<bool>, // true = only on battery, false = only on AC
    #[serde(skip_serializing_if = "Option::is_none")]
    pub if_process_not_running: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub if_time: Option<String>, // e.g., "after 20:00", "before 08:00", "between 09:00-18:00"
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum StepType {
    #[serde(rename = "launch")]
    Launch {
        desktop: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        silent: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        monitor_cond: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        terminal: Option<bool>,
        parallel: bool,
    },
    #[serde(rename = "script")]
    RunScript {
        command: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        dir: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        terminal: Option<bool>,
    },
    #[serde(rename = "wait")]
    Wait {
        ms: u64,
    },
    #[serde(rename = "wait_window")]
    WaitForWindow {
        class: String,
        timeout_ms: u64,
    },
    #[serde(rename = "notify")]
    Notify {
        title: String,
        body: String,
    },
    #[serde(rename = "dispatch")]
    Dispatch {
        command: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkflowStep {
    #[serde(flatten)]
    pub step_type: StepType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<StepCondition>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Workflow {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    pub on_error: FailurePolicy,
    pub steps: Vec<WorkflowStep>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Config {
    pub active_workflow: String,
    pub workflows: HashMap<String, Workflow>,
}

impl Default for Config {
    fn default() -> Self {
        let mut workflows = HashMap::new();
        
        // 1. Default Setup
        workflows.insert(
            "default".to_string(),
            Workflow {
                name: "default".to_string(),
                description: Some("Default developer workstation startup".to_string()),
                project_path: None,
                on_error: FailurePolicy::Continue,
                steps: vec![
                    WorkflowStep {
                        step_type: StepType::Notify {
                            title: "System Startup".to_string(),
                            body: "Launching default developer workspace...".to_string(),
                        },
                        condition: None,
                    },
                    WorkflowStep {
                        step_type: StepType::Launch {
                            desktop: "firefox.desktop".to_string(),
                            workspace: Some("1".to_string()),
                            silent: Some(false),
                            monitor_cond: None,
                            terminal: None,
                            parallel: false,
                        },
                        condition: None,
                    },
                    WorkflowStep {
                        step_type: StepType::Launch {
                            desktop: "kitty.desktop".to_string(),
                            workspace: Some("2".to_string()),
                            silent: Some(true),
                            monitor_cond: None,
                            terminal: None,
                            parallel: false,
                        },
                        condition: None,
                    },
                ],
            },
        );
        
        Config {
            active_workflow: "default".to_string(),
            workflows,
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
    
    let mut file = File::open(&path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    Ok(serde_json::from_str::<Config>(&contents)?)
}

pub fn save_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let path = get_config_path();
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let temp_path = path.with_file_name(format!(".{}.{}.{}.tmp", file_name, std::process::id(), nonce));

    let write_result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }

        let mut file = options.open(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;

        if let Ok(metadata) = std::fs::metadata(&path) {
            std::fs::set_permissions(&temp_path, metadata.permissions())?;
        }
        std::fs::rename(&temp_path, &path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result?;
    Ok(())
}
