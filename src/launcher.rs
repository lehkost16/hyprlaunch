use crate::config::{WorkflowStep, StepType, StepCondition};
use crate::desktop::{find_desktop_file, parse_desktop_file, parse_exec_line, DesktopEntry};
use std::process::Command;
use std::thread;
use std::path::Path;
use std::io::Read;

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

pub fn get_terminal_emulator() -> Option<String> {
    if let Ok(term) = std::env::var("TERMINAL") {
        if !term.is_empty() {
            return Some(term);
        }
    }
    let common_terms = ["kitty", "alacritty", "wezterm", "foot", "ghostty", "gnome-terminal", "konsole", "xfce4-terminal", "xterm"];
    for term in &common_terms {
        if Command::new("which")
            .arg(term)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some(term.to_string());
        }
    }
    None
}

pub fn launch_step(
    desktop: &str,
    workspace: Option<&str>,
    silent: Option<bool>,
    monitor_cond: Option<&str>,
    terminal_opt: Option<bool>,
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
            terminal: false,
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

    let run_in_terminal = terminal_opt.unwrap_or(entry.terminal);

    let final_exec_cmd = if run_in_terminal {
        if let Some(ref term) = get_terminal_emulator() {
            let escaped_full_exec = full_exec_cmd.replace('"', "\\\"");
            format!("{} -e sh -c \"{}\"", term, escaped_full_exec)
        } else {
            full_exec_cmd
        }
    } else {
        full_exec_cmd
    };

    let escaped_lua_cmd = final_exec_cmd
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

    if run_in_terminal {
        println!("Launching {} in terminal...", entry.name);
    } else {
        println!("Launching {}...", entry.name);
    }
    Command::new("hyprctl")
        .args(["dispatch", &hypr_cmd])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    Ok(())
}

pub fn run_script_step(
    command: &str,
    dir: Option<&str>,
    terminal_opt: Option<bool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let run_in_terminal = terminal_opt.unwrap_or(false);

    let final_cmd = if run_in_terminal {
        if let Some(ref term) = get_terminal_emulator() {
            let escaped = command.replace('"', "\\\"");
            format!("{} -e sh -c \"{}\"", term, escaped)
        } else {
            command.to_string()
        }
    } else {
        command.to_string()
    };

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&final_cmd);
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

pub fn dispatch_step(cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Dispatching Hyprland command: {}...", cmd);
    Command::new("hyprctl")
        .args(["dispatch", cmd])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

pub fn is_on_battery() -> bool {
    if let Ok(entries) = std::fs::read_dir("/sys/class/power_supply") {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                if name.starts_with("BAT") {
                    let status_path = path.join("status");
                    if let Ok(mut file) = std::fs::File::open(status_path) {
                        let mut contents = String::new();
                        if file.read_to_string(&mut contents).is_ok() {
                            if contents.trim().eq_ignore_ascii_case("discharging") {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

pub fn is_process_running(name: &str) -> bool {
    let status = Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    status.map(|s| s.success()).unwrap_or(false)
}

pub fn get_local_time() -> Option<(u32, u32)> {
    let output = Command::new("date")
        .arg("+%H:%M")
        .output()
        .ok()?;
    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = text.trim().split(':').collect();
        if parts.len() == 2 {
            let h = parts[0].parse::<u32>().ok()?;
            let m = parts[1].parse::<u32>().ok()?;
            return Some((h, m));
        }
    }
    None
}

pub fn evaluate_time_condition(time_cond: &str) -> bool {
    let (curr_h, curr_m) = match get_local_time() {
        Some(t) => t,
        None => return true, // Fallback if query fails
    };
    let curr_mins = curr_h * 60 + curr_m;
    
    let parse_time = |t_str: &str| -> Option<u32> {
        let parts: Vec<&str> = t_str.trim().split(':').collect();
        if parts.len() == 2 {
            let h = parts[0].parse::<u32>().ok()?;
            let m = parts[1].parse::<u32>().ok()?;
            if h < 24 && m < 60 {
                return Some(h * 60 + m);
            }
        }
        None
    };

    let cond = time_cond.trim();
    if cond.starts_with("after ") {
        let t_str = &cond[6..];
        if let Some(target_mins) = parse_time(t_str) {
            return curr_mins >= target_mins;
        }
    } else if cond.starts_with("before ") {
        let t_str = &cond[7..];
        if let Some(target_mins) = parse_time(t_str) {
            return curr_mins <= target_mins;
        }
    } else if cond.starts_with("between ") {
        let range_str = &cond[8..];
        let range_parts: Vec<&str> = range_str.split('-').collect();
        if range_parts.len() == 2 {
            if let (Some(start_mins), Some(end_mins)) = (parse_time(range_parts[0]), parse_time(range_parts[1])) {
                if start_mins <= end_mins {
                    return curr_mins >= start_mins && curr_mins <= end_mins;
                } else {
                    // Overnight range (e.g., between 22:00-06:00)
                    return curr_mins >= start_mins || curr_mins <= end_mins;
                }
            }
        }
    }
    
    true // Malformed range executes by default
}

pub fn evaluate_step_condition(cond: &StepCondition) -> bool {
    if let Some(on_battery) = cond.if_battery {
        let is_bat = is_on_battery();
        if is_bat != on_battery {
            return false;
        }
    }
    if let Some(ref proc_name) = cond.if_process_not_running {
        if is_process_running(proc_name) {
            return false;
        }
    }
    if let Some(ref time_cond) = cond.if_time {
        if !evaluate_time_condition(time_cond) {
            return false;
        }
    }
    true
}

pub fn execute_step(step: &WorkflowStep) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ref cond) = step.condition {
        if !evaluate_step_condition(cond) {
            println!("Skipping step (condition not met).");
            return Ok(());
        }
    }

    match &step.step_type {
        StepType::Launch { desktop, workspace, silent, monitor_cond, terminal } => {
            launch_step(desktop, workspace.as_deref(), *silent, monitor_cond.as_deref(), *terminal)
        }
        StepType::RunScript { command, dir, terminal } => {
            println!("Running custom command...");
            run_script_step(command, dir.as_deref(), *terminal)
        }
        StepType::Wait { ms } => {
            println!("Waiting {}ms...", ms);
            wait_step(*ms);
            Ok(())
        }
        StepType::Notify { title, body } => {
            println!("Sending notification: \"{}\"...", title);
            notify_step(title, body)
        }
        StepType::Dispatch { command } => {
            dispatch_step(command)
        }
    }
}

#[derive(Debug, Clone)]
pub struct LaunchStatus {
    pub workflow_name: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub step_description: String,
}

pub fn get_launch_status() -> &'static std::sync::Mutex<Option<LaunchStatus>> {
    static STATUS: std::sync::OnceLock<std::sync::Mutex<Option<LaunchStatus>>> = std::sync::OnceLock::new();
    STATUS.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn launch_workflow(workflow_name: &str, steps: &[WorkflowStep]) -> Result<(), Box<dyn std::error::Error>> {
    let total = steps.len();
    for (idx, step) in steps.iter().enumerate() {
        let desc = match &step.step_type {
            StepType::Launch { desktop, .. } => format!("Launching {}", desktop),
            StepType::RunScript { command, .. } => {
                let cmd_short = if command.len() > 30 {
                    format!("{}...", &command[..27])
                } else {
                    command.clone()
                };
                format!("Running script: {}", cmd_short)
            }
            StepType::Wait { ms } => format!("Waiting {}ms", ms),
            StepType::Notify { title, .. } => format!("Notification: {}", title),
            StepType::Dispatch { command } => {
                let cmd_short = if command.len() > 30 {
                    format!("{}...", &command[..27])
                } else {
                    command.clone()
                };
                format!("Dispatch: {}", cmd_short)
            }
        };

        {
            if let Ok(mut status) = get_launch_status().lock() {
                *status = Some(LaunchStatus {
                    workflow_name: workflow_name.to_string(),
                    current_step: idx + 1,
                    total_steps: total,
                    step_description: desc,
                });
            }
        }

        if let Err(e) = execute_step(step) {
            eprintln!("Error executing workflow step: {}", e);
        }
    }

    {
        if let Ok(mut status) = get_launch_status().lock() {
            *status = None;
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
