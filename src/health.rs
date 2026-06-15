use crate::config::WorkflowStep;
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
                // Check if it's executable? For validation, existing is good enough.
                return true;
            }
        }
    }
    false
}

pub fn validate_workflow_steps(steps: &[WorkflowStep]) -> Vec<String> {
    let mut reports = Vec::new();
    for (idx, step) in steps.iter().enumerate() {
        let step_num = idx + 1;
        match step {
            WorkflowStep::Launch { desktop, .. } => {
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
            WorkflowStep::RunScript { command, dir } => {
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
            WorkflowStep::Wait { ms } => {
                if *ms > 10000 {
                    reports.push(format!(
                        "Step {}: Wait duration of {}ms is exceptionally long (suggest < 10000ms).",
                        step_num, ms
                    ));
                }
            }
            WorkflowStep::Notify { title, body } => {
                if title.trim().is_empty() && body.trim().is_empty() {
                    reports.push(format!(
                        "Step {}: Notification has both empty title and empty body.",
                        step_num
                    ));
                }
            }
        }
    }
    reports
}
