use crate::config::{WorkflowStep, StepType, StepCondition};
use crate::desktop::find_desktop_file;
use std::path::Path;

fn is_executable_in_path(name: &str) -> bool {
    if name.starts_with('/') || name.starts_with("./") || name.starts_with("../") {
        return Path::new(name).exists();
    }
    if let Ok(path_env) = std::env::var("PATH") {
        for path_dir in std::env::split_paths(&path_env) {
            let full_path = path_dir.join(name);
            if full_path.is_file() {
                return true;
            }
        }
    }
    false
}

fn validate_time_format(time_str: &str) -> bool {
    let parts: Vec<&str> = time_str.trim().split(':').collect();
    if parts.len() == 2 {
        if let (Ok(h), Ok(m)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            return h < 24 && m < 60;
        }
    }
    false
}

fn validate_step_condition(cond: &StepCondition, step_num: usize, reports: &mut Vec<String>) {
    if let Some(ref time_cond) = cond.if_time {
        let trimmed = time_cond.trim();
        let mut valid = false;
        if trimmed.starts_with("after ") {
            let t = &trimmed[6..];
            valid = validate_time_format(t);
        } else if trimmed.starts_with("before ") {
            let t = &trimmed[7..];
            valid = validate_time_format(t);
        } else if trimmed.starts_with("between ") {
            let r = &trimmed[8..];
            let r_parts: Vec<&str> = r.split('-').collect();
            if r_parts.len() == 2 {
                valid = validate_time_format(r_parts[0]) && validate_time_format(r_parts[1]);
            }
        }
        if !valid {
            reports.push(format!(
                "Step {}: Time condition '{}' is malformed. Use 'after HH:MM', 'before HH:MM', or 'between HH:MM-HH:MM'.",
                step_num, time_cond
            ));
        }
    }
    if let Some(ref proc_name) = cond.if_process_not_running {
        if proc_name.trim().is_empty() {
            reports.push(format!(
                "Step {}: Condition 'process not running' has an empty process name.",
                step_num
            ));
        }
    }
}

pub fn validate_workflow_steps(steps: &[WorkflowStep]) -> Vec<String> {
    let mut reports = Vec::new();
    for (idx, step) in steps.iter().enumerate() {
        let step_num = idx + 1;
        
        // 1. Validate step condition
        if let Some(ref cond) = step.condition {
            validate_step_condition(cond, step_num, &mut reports);
        }
        
        // 2. Validate step type payload
        match &step.step_type {
            StepType::Launch { desktop, .. } => {
                if desktop.ends_with(".desktop") {
                    if find_desktop_file(desktop).is_none() {
                        reports.push(format!(
                            "Step {}: Desktop application file '{}' not found on system.",
                            step_num, desktop
                        ));
                    }
                } else {
                    let exec_name = desktop.split_whitespace().next().unwrap_or(desktop);
                    if !is_executable_in_path(exec_name) {
                        reports.push(format!(
                            "Step {}: Custom command executable '{}' not found in PATH.",
                            step_num, exec_name
                        ));
                    }
                }
            }
            StepType::RunScript { command, dir, .. } => {
                if let Some(d) = dir {
                    let p = Path::new(d);
                    if !p.exists() {
                        reports.push(format!(
                            "Step {}: Script working directory '{}' does not exist.",
                            step_num, d
                        ));
                    } else if !p.is_dir() {
                        reports.push(format!(
                            "Step {}: Script working directory '{}' is not a directory.",
                            step_num, d
                        ));
                    }
                }
                
                let first_word = command.split_whitespace().next().unwrap_or(command);
                if first_word.starts_with('/') || first_word.starts_with("./") || first_word.starts_with("../") {
                    let p = Path::new(first_word);
                    if !p.exists() {
                        reports.push(format!(
                            "Step {}: Script file path '{}' does not exist.",
                            step_num, first_word
                        ));
                    }
                }
            }
            StepType::Wait { ms } => {
                if *ms > 10000 {
                    reports.push(format!(
                        "Step {}: Wait duration of {}ms is exceptionally long (suggest < 10000ms).",
                        step_num, ms
                    ));
                }
            }
            StepType::Notify { title, body } => {
                if title.trim().is_empty() && body.trim().is_empty() {
                    reports.push(format!(
                        "Step {}: Notification has both empty title and empty body.",
                        step_num
                    ));
                }
            }
            StepType::Dispatch { command } => {
                if command.trim().is_empty() {
                    reports.push(format!(
                        "Step {}: Hyprland dispatch command is empty.",
                        step_num
                    ));
                }
            }
        }
    }
    reports
}
