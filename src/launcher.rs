use std::sync::atomic::{AtomicBool, Ordering};

static IS_TUI: AtomicBool = AtomicBool::new(false);

pub fn set_is_tui(val: bool) {
    IS_TUI.store(val, Ordering::Relaxed);
}

pub fn is_tui() -> bool {
    IS_TUI.load(Ordering::Relaxed)
}

macro_rules! println {
    ($($arg:tt)*) => {
        if !crate::launcher::is_tui() {
            std::println!($($arg)*);
        }
    };
}

macro_rules! eprintln {
    ($($arg:tt)*) => {
        if !crate::launcher::is_tui() {
            std::eprintln!($($arg)*);
        }
    };
}

use crate::config::{FailurePolicy, WorkflowStep, StepType, StepCondition};
use crate::desktop::{find_desktop_file, parse_desktop_file, parse_exec_line, DesktopEntry};
use std::process::Command;
use std::thread;
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;

pub fn hyprctl_cmd() -> Command {
    let mut cmd = Command::new("hyprctl");
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        if let Ok(xdg_runtime) = std::env::var("XDG_RUNTIME_DIR") {
            let hypr_dir = Path::new(&xdg_runtime).join("hypr");
            if let Ok(entries) = std::fs::read_dir(&hypr_dir) {
                let mut best_sig: Option<(std::time::SystemTime, String)> = None;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let socket_path = path.join(".socket.sock");
                        if socket_path.exists() {
                            if let Ok(meta) = std::fs::metadata(&socket_path) {
                                if let Ok(mtime) = meta.modified() {
                                    if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                                        if best_sig.as_ref().map_or(true, |(best_time, _)| mtime > *best_time) {
                                            best_sig = Some((mtime, file_name.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some((_, sig)) = best_sig {
                    cmd.env("HYPRLAND_INSTANCE_SIGNATURE", sig);
                }
            }
        }
    }
    cmd
}

pub fn get_socket2_path() -> Option<PathBuf> {
    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let hypr_dir = Path::new(&xdg_runtime).join("hypr");

    if let Ok(sig) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        let sock = hypr_dir.join(&sig).join(".socket2.sock");
        if sock.exists() {
            return Some(sock);
        }
    }

    if let Ok(entries) = std::fs::read_dir(&hypr_dir) {
        let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let sock = p.join(".socket2.sock");
                if sock.exists() {
                    if let Ok(meta) = std::fs::metadata(&sock) {
                        if let Ok(mtime) = meta.modified() {
                            if best.as_ref().map_or(true, |(best_time, _)| mtime > *best_time) {
                                best = Some((mtime, sock));
                            }
                        }
                    }
                }
            }
        }
        if let Some((_, sock)) = best {
            return Some(sock);
        }
    }

    None
}

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

fn get_bin_name(exec: &[String]) -> Option<String> {
    if exec.is_empty() { return None; }
    for arg in exec {
        let path = Path::new(arg);
        if let Some(basename) = path.file_name().and_then(|s| s.to_str()) {
            if basename == "env" || basename == "sh" || basename == "bash" || basename == "flatpak" {
                continue;
            }
            if basename.starts_with('-') || basename.contains('=') {
                continue;
            }
            return Some(basename.to_string());
        }
    }
    Path::new(&exec[0]).file_name().and_then(|s| s.to_str()).map(|s| s.to_string())
}

pub fn find_matching_windows(entry: &DesktopEntry) -> Vec<String> {
    let output = hyprctl_cmd()
        .args(["clients", "-j"])
        .output();
    
    let mut matching_addresses = Vec::new();
    if let Ok(out) = output {
        if out.status.success() {
            if let Ok(json_str) = std::str::from_utf8(&out.stdout) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(arr) = value.as_array() {
                        let wm_class = entry.startup_wm_class.as_deref().unwrap_or("").to_lowercase();
                        let desktop_id = entry.filename.strip_suffix(".desktop")
                            .unwrap_or(&entry.filename)
                            .to_lowercase();
                        
                        let bin_name = get_bin_name(&entry.exec)
                            .unwrap_or_default()
                            .to_lowercase();
                        
                        for client in arr {
                            let class = client.get("class")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_lowercase();
                            let initial_class = client.get("initialClass")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_lowercase();
                            
                            let clean_class = class.split('.').last().unwrap_or(&class).to_lowercase();
                            let clean_initial_class = initial_class.split('.').last().unwrap_or(&initial_class).to_lowercase();
                            
                            let is_match = (!wm_class.is_empty() && (class == wm_class || initial_class == wm_class))
                                || (class == desktop_id || initial_class == desktop_id)
                                || (!bin_name.is_empty() && (class == bin_name || initial_class == bin_name || clean_class == bin_name || clean_initial_class == bin_name));
                            
                            if is_match {
                                if let Some(addr) = client.get("address").and_then(|v| v.as_str()) {
                                    matching_addresses.push(addr.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    matching_addresses
}

fn is_terminal_app(entry: &DesktopEntry) -> bool {
    if entry.terminal {
        return true;
    }
    let bin_name = get_bin_name(&entry.exec)
        .unwrap_or_default()
        .to_lowercase();
    let common_terms = ["kitty", "alacritty", "wezterm", "foot", "ghostty", "gnome-terminal", "konsole", "xfce4-terminal", "xterm"];
    common_terms.contains(&bin_name.as_str())
}

pub fn get_active_workspace() -> String {
    let output = hyprctl_cmd()
        .args(["activeworkspace", "-j"])
        .output();
    if let Ok(out) = output {
        if out.status.success() {
            if let Ok(json_str) = std::str::from_utf8(&out.stdout) {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(id) = value.get("id").and_then(|v| v.as_i64()) {
                        return id.to_string();
                    }
                    if let Some(name) = value.get("name").and_then(|v| v.as_str()) {
                        return name.to_string();
                    }
                }
            }
        }
    }
    
    // Fallback: parse plain text output
    let output = hyprctl_cmd()
        .arg("activeworkspace")
        .output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.starts_with("workspace ID") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() > 2 {
                    return parts[2].to_string();
                }
            }
        }
    }
    "1".to_string() // Ultimate fallback
}

pub fn get_active_monitors() -> Vec<String> {
    // Try to parse using hyprctl monitors -j
    let output = hyprctl_cmd()
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
    let output = hyprctl_cmd()
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
    let active_monitors = get_active_monitors();
    let resolved_ws = resolve_workspace(workspace, monitor_cond, &active_monitors);

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
            startup_wm_class: None,
        }
    };

    let matching_windows = find_matching_windows(&entry);
    let app_was_running = !matching_windows.is_empty();

    if app_was_running {
        if !is_terminal_app(&entry) {
            if let Some(ref ws) = resolved_ws {
                println!("Application '{}' is already running. Moving its windows to workspace {} silently.", entry.name, ws);
                for addr in matching_windows {
                    let move_cmd = format!(
                        "hl.dsp.window.move({{ workspace = \"{}\", follow = false, window = \"address:{}\" }})",
                        ws, addr
                    );
                    let _ = hyprctl_cmd()
                        .args(["dispatch", &move_cmd])
                        .status();
                }
            } else {
                println!("Application '{}' is already running. No workspace configured, skipping.", entry.name);
            }
            return Ok(());
        }
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


    let hypr_cmd = if resolved_ws.is_some() || silent.unwrap_or(false) {
        let ws_str = resolved_ws.as_deref().unwrap_or("");
        let silent_flag = if silent.unwrap_or(false) {
            if ws_str.is_empty() { "silent" } else { " silent" }
        } else {
            ""
        };
        let rule_prefix = if ws_str.is_empty() {
            "silent".to_string()
        } else {
            format!("workspace {}{}", ws_str, silent_flag)
        };
        format!(
            "hl.dsp.exec_cmd(\"[{}] {}\")",
            rule_prefix, escaped_lua_cmd
        )
    } else {
        format!("hl.dsp.exec_cmd(\"{}\")", escaped_lua_cmd)
    };

    if run_in_terminal {
        println!("Launching {} in terminal...", entry.name);
    } else {
        println!("Launching {}...", entry.name);
    }
    println!("Debug: hyprctl dispatch {}", hypr_cmd);
    let output = hyprctl_cmd()
        .args(["dispatch", &hypr_cmd])
        .output()?;
    println!("Debug: hyprctl stdout: {}", String::from_utf8_lossy(&output.stdout));
    println!("Debug: hyprctl stderr: {}", String::from_utf8_lossy(&output.stderr));

    if let Some(ref ws) = resolved_ws {
        let entry_clone = entry.clone();
        let ws_clone = ws.clone();
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            let duration = std::time::Duration::from_secs(8);
            
            // Record already existing window addresses to only move new windows
            let mut existing_addresses = std::collections::HashSet::new();
            for addr in find_matching_windows(&entry_clone) {
                existing_addresses.insert(addr);
            }

            let wm_class = entry_clone.startup_wm_class.as_deref().unwrap_or("").to_lowercase();
            let desktop_id = entry_clone.filename.strip_suffix(".desktop")
                .unwrap_or(&entry_clone.filename)
                .to_lowercase();
            let bin_name = get_bin_name(&entry_clone.exec)
                .unwrap_or_default()
                .to_lowercase();

            let matches_target = |class: &str, title: &str| -> bool {
                let cl = class.to_lowercase();
                let ti = title.to_lowercase();
                (!wm_class.is_empty() && (cl == wm_class || ti.contains(&wm_class)))
                    || (!desktop_id.is_empty() && (cl == desktop_id || ti.contains(&desktop_id)))
                    || (!bin_name.is_empty() && (cl == bin_name || ti.contains(&bin_name)))
            };

            // Fast path: try native Socket2 event stream
            if let Some(sock_path) = get_socket2_path() {
                if let Ok(stream) = UnixStream::connect(&sock_path) {
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(300)));
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();

                    while start.elapsed() < duration {
                        line.clear();
                        match reader.read_line(&mut line) {
                            Ok(0) => break, // EOF
                            Ok(_) => {
                                let trimmed = line.trim();
                                if trimmed.starts_with("openwindow>>") {
                                    let payload = &trimmed["openwindow>>".len()..];
                                    let parts: Vec<&str> = payload.split(',').collect();
                                    if parts.len() >= 4 {
                                        let addr = format!("0x{}", parts[0].trim_start_matches("0x"));
                                        let class = parts[2];
                                        let title = parts[3..].join(",");
                                        if matches_target(class, &title) && !existing_addresses.contains(&addr) {
                                            println!("Background Socket2: caught new window {} ({}) -> moving to workspace {}", addr, class, ws_clone);
                                            let move_cmd = format!(
                                                "hl.dsp.window.move({{ workspace = \"{}\", follow = false, window = \"address:{}\" }})",
                                                ws_clone, addr
                                            );
                                            let _ = hyprctl_cmd()
                                                .args(["dispatch", &move_cmd])
                                                .status();
                                            existing_addresses.insert(addr);
                                        }
                                    }
                                }
                            }
                            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                                // Timeout tick, check loop duration
                                continue;
                            }
                            Err(_) => break, // Fallback to polling
                        }
                    }
                    return;
                }
            }

            // Fallback path: polling if Socket2 was unavailable
            let poll_interval = std::time::Duration::from_millis(150);
            while start.elapsed() < duration {
                std::thread::sleep(poll_interval);
                let current_windows = find_matching_windows(&entry_clone);
                for addr in current_windows {
                    if !existing_addresses.contains(&addr) {
                        println!("Background monitor: moving new window {} silently to workspace {}", addr, ws_clone);
                        let move_cmd = format!(
                            "hl.dsp.window.move({{ workspace = \"{}\", follow = false, window = \"address:{}\" }})",
                            ws_clone, addr
                        );
                        let _ = hyprctl_cmd()
                            .args(["dispatch", &move_cmd])
                            .status();
                        existing_addresses.insert(addr);
                    }
                }
            }
        });
    }

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
    hyprctl_cmd()
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

    execute_step_unchecked(step)
}

fn execute_step_unchecked(step: &WorkflowStep) -> Result<(), Box<dyn std::error::Error>> {
    match &step.step_type {
        StepType::Launch { desktop, workspace, silent, monitor_cond, terminal, .. } => {
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
        StepType::WaitForWindow { class, timeout_ms } => wait_for_window(class, *timeout_ms),
        StepType::Notify { title, body } => {
            println!("Sending notification: \"{}\"...", title);
            notify_step(title, body)
        }
        StepType::Dispatch { command } => {
            dispatch_step(command)
        }
    }
}

fn wait_for_window(class: &str, timeout_ms: u64) -> Result<(), Box<dyn std::error::Error>> {
    let expected = class.trim();
    if expected.is_empty() {
        return Err("Window class cannot be empty".into());
    }

    println!("Waiting for window class '{}' (timeout {}ms)...", expected, timeout_ms);
    let timeout = std::time::Duration::from_millis(timeout_ms);
    let started = std::time::Instant::now();

    // Fast check: is the window already present?
    let check_existing = || -> bool {
        if let Ok(output) = hyprctl_cmd().args(["clients", "-j"]).output() {
            if output.status.success() {
                if let Ok(clients) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                    if let Some(arr) = clients.as_array() {
                        return arr.iter().any(|client| {
                            ["class", "initialClass"].iter().any(|field| {
                                client.get(field)
                                    .and_then(serde_json::Value::as_str)
                                    .map_or(false, |value| value.eq_ignore_ascii_case(expected))
                            })
                        });
                    }
                }
            }
        }
        false
    };

    if check_existing() {
        return Ok(());
    }

    // Event-driven path: listen on Socket2
    if let Some(sock_path) = get_socket2_path() {
        if let Ok(stream) = UnixStream::connect(&sock_path) {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(200)));
            let mut reader = BufReader::new(stream);
            let mut line = String::new();

            while started.elapsed() < timeout {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.starts_with("openwindow>>") {
                            let payload = &trimmed["openwindow>>".len()..];
                            let parts: Vec<&str> = payload.split(',').collect();
                            if parts.len() >= 3 {
                                let window_class = parts[2];
                                if window_class.eq_ignore_ascii_case(expected) {
                                    return Ok(());
                                }
                            }
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                        continue;
                    }
                    Err(_) => break, // Fallback
                }
            }
            if started.elapsed() >= timeout {
                return Err(format!("Timed out waiting for window class '{}' after {}ms", expected, timeout_ms).into());
            }
        }
    }

    // Fallback polling loop
    loop {
        if check_existing() {
            return Ok(());
        }
        let elapsed = started.elapsed();
        if elapsed >= timeout {
            return Err(format!("Timed out waiting for window class '{}' after {}ms", expected, timeout_ms).into());
        }
        thread::sleep(std::time::Duration::from_millis(150).min(timeout.saturating_sub(elapsed)));
    }
}

#[derive(Debug, Clone)]
pub struct LaunchStatus {
    pub workflow_name: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub step_description: String,
    pub run_state: WorkflowRunState,
    pub step_states: Vec<StepRunState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowRunState {
    Running,
    Completed,
    CompletedWithErrors,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepRunState {
    Pending,
    Running,
    Succeeded,
    Skipped,
    Failed(String),
}

pub fn get_launch_status() -> &'static std::sync::Mutex<Option<LaunchStatus>> {
    static STATUS: std::sync::OnceLock<std::sync::Mutex<Option<LaunchStatus>>> = std::sync::OnceLock::new();
    STATUS.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn is_workflow_running() -> bool {
    get_launch_status()
        .lock()
        .ok()
        .and_then(|status| status.as_ref().map(|s| s.run_state == WorkflowRunState::Running))
        .unwrap_or(false)
}

pub fn launch_workflow(workflow_name: &str, steps: &[WorkflowStep]) -> Result<(), Box<dyn std::error::Error>> {
    launch_workflow_with_policy(workflow_name, steps, FailurePolicy::Continue)
}

pub fn launch_workflow_with_policy(
    workflow_name: &str,
    steps: &[WorkflowStep],
    on_error: FailurePolicy,
) -> Result<(), Box<dyn std::error::Error>> {
    let total = steps.len();

    {
        let mut status = get_launch_status()
            .lock()
            .map_err(|_| "Workflow status lock is unavailable")?;
        if status
            .as_ref()
            .map_or(false, |s| s.run_state == WorkflowRunState::Running)
        {
            return Err("A workflow is already running".into());
        }
        *status = Some(LaunchStatus {
            workflow_name: workflow_name.to_string(),
            current_step: 0,
            total_steps: total,
            step_description: "Preparing workflow".to_string(),
            run_state: WorkflowRunState::Running,
            step_states: vec![StepRunState::Pending; total],
        });
    }

    let mut had_errors = false;
    let mut idx = 0;
    while idx < steps.len() {
        let step = &steps[idx];
        let is_parallel_launch = matches!(
            &step.step_type,
            StepType::Launch { parallel: true, .. }
        );
        let end = if is_parallel_launch {
            let mut end = idx + 1;
            while end < steps.len()
                && matches!(&steps[end].step_type, StepType::Launch { parallel: true, .. })
            {
                end += 1;
            }
            end
        } else {
            idx + 1
        };

        let group = &steps[idx..end];
        let mut runnable = Vec::new();
        for (offset, grouped_step) in group.iter().enumerate() {
            let step_idx = idx + offset;
            let condition_met = grouped_step.condition.as_ref().map(evaluate_step_condition).unwrap_or(true);
            if let Ok(mut status) = get_launch_status().lock() {
                if let Some(status) = status.as_mut() {
                    status.current_step = step_idx + 1;
                    status.step_description = match &grouped_step.step_type {
                        StepType::Launch { desktop, .. } if group.len() > 1 => format!("Launching {} (parallel group)", desktop),
                        _ => desc_for_step(grouped_step),
                    };
                    status.step_states[step_idx] = if condition_met { StepRunState::Running } else { StepRunState::Skipped };
                }
            }
            if condition_met {
                runnable.push((step_idx, grouped_step));
            }
        }

        if is_parallel_launch && runnable.len() > 1 {
            let results = thread::scope(|scope| {
                let handles: Vec<_> = runnable.iter().map(|(step_idx, grouped_step)| {
                    let step_idx = *step_idx;
                    let grouped_step = *grouped_step;
                    (step_idx, scope.spawn(move || execute_step_unchecked(grouped_step).map_err(|error| error.to_string())))
                }).collect();
                handles.into_iter().map(|(step_idx, handle)| {
                    let result = handle.join().unwrap_or_else(|_| Err("Step thread panicked".to_string()));
                    (step_idx, result)
                }).collect::<Vec<_>>()
            });
            for (step_idx, result) in results {
                match result {
                    Ok(()) => set_step_state(step_idx, StepRunState::Succeeded),
                    Err(error) => {
                        had_errors = true;
                        eprintln!("Error executing workflow step: {}", error);
                        set_step_state(step_idx, StepRunState::Failed(error));
                    }
                }
            }
        } else if let Some((step_idx, grouped_step)) = runnable.first() {
            match execute_step_unchecked(grouped_step) {
                Ok(()) => set_step_state(*step_idx, StepRunState::Succeeded),
                Err(error) => {
                    had_errors = true;
                    eprintln!("Error executing workflow step: {}", error);
                    set_step_state(*step_idx, StepRunState::Failed(error.to_string()));
                }
            }
        }

        idx = end;
        if had_errors && on_error == FailurePolicy::Stop {
            if let Ok(mut status) = get_launch_status().lock() {
                if let Some(status) = status.as_mut() {
                    for state in status.step_states.iter_mut().skip(idx) {
                        *state = StepRunState::Skipped;
                    }
                }
            }
            break;
        }
    }

    if let Ok(mut status) = get_launch_status().lock() {
        if let Some(status) = status.as_mut() {
            status.current_step = total;
            status.step_description = if had_errors {
                "Workflow finished with errors".to_string()
            } else {
                "Workflow finished successfully".to_string()
            };
            status.run_state = if had_errors {
                WorkflowRunState::CompletedWithErrors
            } else {
                WorkflowRunState::Completed
            };
        }
    }
    Ok(())
}

fn desc_for_step(step: &WorkflowStep) -> String {
    match &step.step_type {
        StepType::Launch { desktop, .. } => format!("Launching {}", desktop),
        StepType::RunScript { command, .. } => format!("Running script: {}", command.chars().take(30).collect::<String>()),
        StepType::Wait { ms } => format!("Waiting {}ms", ms),
        StepType::WaitForWindow { class, .. } => format!("Waiting for window: {}", class),
        StepType::Notify { title, .. } => format!("Notification: {}", title),
        StepType::Dispatch { command } => format!("Dispatch: {}", command.chars().take(30).collect::<String>()),
    }
}

fn set_step_state(index: usize, state: StepRunState) {
    if let Ok(mut status) = get_launch_status().lock() {
        if let Some(status) = status.as_mut() {
            status.current_step = index + 1;
            status.step_states[index] = state;
        }
    }
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
