use crate::config::ProfileApp;
use crate::desktop::{find_desktop_file, parse_desktop_file, DesktopEntry};
use std::process::Command;
use std::thread;
use std::path::Path;
use ratatui::widgets::ListState;

pub struct AppUiState{
    pub name: String,
    pub config: ProfileApp,
    pub is_active: bool,
}

pub struct TuiApplications{
    pub apps: Vec<AppUiState>,
    pub list_state: ListState,
    pub should_quit: bool,
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

pub fn launch_app_now(app: &ProfileApp) -> Result<(), Box<dyn std::error::Error>> {
    let path = match find_desktop_file(&app.desktop) {
        Some(p) => p,
        None => {
            return Err(format!("Could not find desktop file for: {}", app.desktop).into());
        }
    };

    let entry = match parse_desktop_file(&path) {
        Some(e) => e,
        None => {
            return Err(format!("Failed to parse desktop file at: {:?}", path).into());
        }
    };

    if is_app_running(&entry) {
        println!("Application '{}' (from {}) is already running. Skipping launch.", entry.name, app.desktop);
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

    let hypr_cmd = if let Some(ref ws) = app.workspace {
        let silent_flag = if app.silent.unwrap_or(false) { " silent" } else { "" };
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

pub fn launch_profile(apps: &[ProfileApp]) -> Result<(), Box<dyn std::error::Error>> {
    let mut handles = Vec::new();
    
    for app in apps {
        let app = app.clone();
        let handle = thread::spawn(move || {
            if let Err(e) = launch_app_now(&app) {
                eprintln!("Error launching {}: {}", app.desktop, e);
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        let _ = handle.join();
    }
    
    Ok(())
}
