use std::env;
use std::io::IsTerminal;

pub mod config;
pub mod desktop;
pub mod launcher;
pub mod tui;
pub mod gui;

fn print_help() {
    println!("hyprlaunch - A simple, profile-based application launcher for Hyprland");
    println!();
    println!("Usage:");
    println!("  hyprlaunch                 Open the interactive configuration TUI (if in terminal)");
    println!("                             or launch the active profile (if non-interactive)");
    println!("  hyprlaunch tui             Explicitly open the configuration TUI");
    println!("  hyprlaunch gui             Open the native desktop configuration GUI");
    println!("  hyprlaunch list            List all available profile names");
    println!("  hyprlaunch <profile_name>  Launch applications for the specified profile");
    println!("  hyprlaunch --help | -h     Print this help message");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    if args.is_empty() {
        if std::io::stdout().is_terminal() {
            tui::run_tui()?;
        } else {
            let cfg = config::load_config()?;
            if let Some(apps) = cfg.profiles.get(&cfg.active_profile) {
                launcher::launch_profile(apps)?;
            } else {
                eprintln!("Active profile '{}' not found in config.", cfg.active_profile);
            }
        }
    } else {
        match args[0].as_str() {
            "tui" => {
                tui::run_tui()?;
            }
            "gui" => {
                gui::run_gui()?;
            }
            "list" => {
                let cfg = config::load_config()?;
                println!("Available profiles:");
                let mut profiles: Vec<&String> = cfg.profiles.keys().collect();
                profiles.sort();
                for p in profiles {
                    if *p == cfg.active_profile {
                        println!(" - {} (active)", p);
                    } else {
                        println!(" - {}", p);
                    }
                }
            }
            profile_name => {
                let cfg = config::load_config()?;
                if let Some(apps) = cfg.profiles.get(profile_name) {
                    println!("Launching profile: {}", profile_name);
                    launcher::launch_profile(apps)?;
                } else {
                    eprintln!("Profile '{}' not found in config.", profile_name);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}