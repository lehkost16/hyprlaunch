use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

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
    
    // 1. Try to parse as new Config format
    if let Ok(config) = serde_json::from_str::<Config>(&contents) {
        return Ok(config);
    }
    
    // 2. Try to parse as old config format and migrate
    #[derive(Deserialize)]
    struct OldProfileApp {
        desktop: String,
        workspace: Option<String>,
        silent: Option<bool>,
    }
    #[derive(Deserialize)]
    struct OldConfig {
        active_profile: String,
        profiles: HashMap<String, Vec<OldProfileApp>>,
    }
    
    if let Ok(old_config) = serde_json::from_str::<OldConfig>(&contents) {
        println!("Migrating legacy hyprlaunch configuration to workflow orchestrator format...");
        let mut workflows = HashMap::new();
        for (profile_name, apps) in old_config.profiles {
            let steps = apps.into_iter().map(|app| {
                WorkflowStep {
                    step_type: StepType::Launch {
                        desktop: app.desktop,
                        workspace: app.workspace,
                        silent: app.silent,
                        monitor_cond: None,
                        terminal: None,
                    },
                    condition: None,
                }
            }).collect();
            workflows.insert(profile_name.clone(), Workflow {
                name: profile_name,
                description: None,
                project_path: None,
                steps,
            });
        }
        let migrated_config = Config {
            active_workflow: old_config.active_profile,
            workflows,
        };
        save_config(&migrated_config)?;
        return Ok(migrated_config);
    }
    
    Err("Failed to parse configuration file (invalid format)".into())
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
