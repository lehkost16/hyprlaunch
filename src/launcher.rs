use crate::config::WorkflowStep;
use crate::desktop::{find_desktop_file, parse_desktop_file, parse_exec_line, DesktopEntry};
use std::process::Command;
use std::thread;
use std::path::Path;

pub fn is_app_running(entry: &DesktopEntry) -> bool {
    if entry.exec.is_empty() {
        return false;
    }

    let bin = &entry.exec[0];
    let basename = Path::new(bin)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(bin);

    if basename == "flatpak" && entry.exec.len() > 1 {
        // For flatpak, look for the app ID in the command line (e.g., flatpak run org.foo.bar)
        let app_id = entry.exec.iter()
            .find(|arg| arg.contains('.') && !arg.starts_with('-'))
            .cloned()
            .unwrap_or_else(|| entry.exec[entry.exec.len() - 1].clone());

        let status = Command::new("pgrep")
            .arg("-f")
            .arg(&app_id)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        return status.map(|s| s.success()).unwrap_or(false);
    }

    // Default check: pgrep -x <basename>
    let status = Command::new("pgrep")
        .arg("-x")
        .arg(basename)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    status.map(|s| s.success()).unwrap_or(false)
}

pub fn get_active_monitors() -> Vec<String> {
    // Try to parse using hyprctl monitors -j
    let output = Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output();
    
    if let Ok(out) = output {
        if out.status.success() {
            if let Ok(json_str) = std::str::from_utf8(&out.stdout) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(arr) = value.as_array() {
                        return arr.iter()
                            .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                            .collect();
                    }
                }
            }
        }
    }
    
    // Fallback: parse plain text output of hyprctl monitors
    let output = Command::new("hyprctl")
        .arg("monitors")
        .output();
    
    let mut monitors = Vec::new();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.starts_with("Monitor ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 1 {
                    // Strip trailing colon or paren if any
                    let name = parts[1].trim_end_matches(':').trim_end_matches('(');
                    monitors.push(name.to_string());
                }
            }
        }
    }
    monitors
}

pub fn resolve_workspace(
    workspace: Option<&str>,
    monitor_cond: Option<&str>,
    active_monitors: &[String],
) -> Option<String> {
    if let Some(cond) = monitor_cond {
        // Syntax: "HDMI-A-1?3:1"
        if let Some(q_pos) = cond.find('?') {
            let monitor = cond[..q_pos].trim();
            let remainder = &cond[q_pos + 1..];
            if let Some(colon_pos) = remainder.find(':') {
                let true_ws = remainder[..colon_pos].trim();
                let false_ws = remainder[colon_pos + 1..].trim();
                let is_connected = active_monitors.iter().any(|m| m == monitor);
                let resolved = if is_connected { true_ws } else { false_ws };
                return Some(resolved.to_string());
            }
        }
    }
    workspace.map(|w| w.to_string())
}

pub fn launch_step(
    desktop: &str,
    workspace: Option<&str>,
    silent: Option<bool>,
    monitor_cond: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let entry = if let Some(path) = find_desktop_file(desktop) {
        match parse_desktop_file(&path) {
            Some(e) => e,
            None => {
                return Err(format!("Failed to parse desktop file at: {:?}", path).into());
            }
        }
    } else {
        let exec_args = parse_exec_line(desktop)
            .unwrap_or_else(|| vec![desktop.to_string()]);
        DesktopEntry {
            filename: desktop.to_string(),
            file_path: std::path::PathBuf::new(),
            name: "Custom Command".to_string(),
            exec: exec_args,
            path: None,
            hidden: false,
        }
    };

    if is_app_running(&entry) {
        println!("Application '{}' (from {}) is already running. Skipping launch.", entry.name, desktop);
        return Ok(());
    }

    if entry.exec.is_empty() {
        return Err("Desktop file has an empty Exec command".into());
    }

    let mut cmd_parts = Vec::new();
    for arg in &entry.exec {
        if arg.contains(' ') || arg.contains('"') || arg.contains('\'') {
            let escaped = arg.replace('\'', "'\\''");
            cmd_parts.push(format!("'{}'", escaped));
        } else {
            cmd_parts.push(arg.clone());
        }
    }
    let command_parts_joined = cmd_parts.join(" ");

    let full_exec_cmd = if let Some(ref work_dir) = entry.path {
        let escaped_work_dir = work_dir.replace('\'', "'\\''");
        format!("sh -c \"cd '{}' && exec {}\"", escaped_work_dir, command_parts_joined)
    } else {
        command_parts_joined
    };

    let escaped_lua_cmd = full_exec_cmd
        .replace('\\', "\\\\")
        .replace('"', "\\\"");

    let active_monitors = get_active_monitors();
    let resolved_ws = resolve_workspace(workspace, monitor_cond, &active_monitors);

    let hypr_cmd = if let Some(ref ws) = resolved_ws {
        let silent_flag = if silent.unwrap_or(false) { " silent" } else { "" };
        format!(
            "hl.dsp.exec_cmd(\"{}\", {{ workspace = \"{}{}\" }})",
            escaped_lua_cmd, ws, silent_flag
        )
    } else {
        format!("hl.dsp.exec_cmd(\"{}\")", escaped_lua_cmd)
    };

    println!("Launching {}: {}", entry.name, hypr_cmd);
    Command::new("hyprctl")
        .args(["dispatch", &hypr_cmd])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    Ok(())
}

pub fn run_script_step(command: &str, dir: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    cmd.stdout(std::process::Stdio::null())
       .stderr(std::process::Stdio::null())
       .spawn()?;
    Ok(())
}

pub fn wait_step(ms: u64) {
    thread::sleep(std::time::Duration::from_millis(ms));
}

pub fn notify_step(title: &str, body: &str) -> Result<(), Box<dyn std::error::Error>> {
    Command::new("notify-send")
        .arg(title)
        .arg(body)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

pub fn execute_step(step: &WorkflowStep) -> Result<(), Box<dyn std::error::Error>> {
    match step {
        WorkflowStep::Launch { desktop, workspace, silent, monitor_cond } => {
            launch_step(desktop, workspace.as_deref(), *silent, monitor_cond.as_deref())
        }
        WorkflowStep::RunScript { command, dir } => {
            run_script_step(command, dir.as_deref())
        }
        WorkflowStep::Wait { ms } => {
            wait_step(*ms);
            Ok(())
        }
        WorkflowStep::Notify { title, body } => {
            notify_step(title, body)
        }
    }
}

pub fn launch_workflow(steps: &[WorkflowStep]) -> Result<(), Box<dyn std::error::Error>> {
    for step in steps {
        if let Err(e) = execute_step(step) {
            eprintln!("Error executing workflow step: {}", e);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_workspace() {
        let active = vec!["HDMI-A-1".to_string(), "eDP-1".to_string()];
        
        // Match when monitor connected
        assert_eq!(
            resolve_workspace(Some("1"), Some("HDMI-A-1?3:1"), &active),
            Some("3".to_string())
        );
        
        // Match fallback when monitor not connected
        assert_eq!(
            resolve_workspace(Some("1"), Some("DP-1?3:1"), &active),
            Some("1".to_string())
        );
        
        // Fall back to workspace when no condition is provided
        assert_eq!(
            resolve_workspace(Some("5"), None, &active),
            Some("5".to_string())
        );
        
        // Handle no workspace and no condition
        assert_eq!(
            resolve_workspace(None, None, &active),
            None
        );
    }
}

