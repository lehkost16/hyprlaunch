use crate::config::ProfileApp;
use crate::desktop::{find_desktop_file, parse_desktop_file};
use std::process::Command;
use std::thread;
use std::time::Duration;

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
            if let Some(delay) = app.delay_ms {
                if delay > 0 {
                    println!("Scheduling {} launch with {}ms delay...", app.desktop, delay);
                    thread::sleep(Duration::from_millis(delay));
                }
            }
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
