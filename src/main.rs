use std::env;
use std::io::IsTerminal;

pub mod config;
pub mod desktop;
pub mod health;
pub mod launcher;
pub mod tui;

fn print_help() {
    println!("hyprlaunch - Workflow Orchestrator for Hyprland");
    println!();
    println!("Usage:");
    println!("  hyprlaunch                 Open the interactive configuration TUI (if in terminal)");
    println!("                             or launch the active workflow (if non-interactive)");
    println!("  hyprlaunch tui             Explicitly open the configuration TUI");
    println!("  hyprlaunch list            List all available workflow names");
    println!("  hyprlaunch <workflow_name> Launch the specified workflow");
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
            if let Some(wf) = cfg.workflows.get(&cfg.active_workflow) {
                launcher::launch_workflow(&cfg.active_workflow, &wf.steps)?;
            } else {
                eprintln!("Active workflow '{}' not found in config.", cfg.active_workflow);
            }
        }
    } else {
        match args[0].as_str() {
            "tui" => {
                tui::run_tui()?;
            }
            "list" => {
                let cfg = config::load_config()?;
                println!("Available workflows:");
                let mut workflows: Vec<&String> = cfg.workflows.keys().collect();
                workflows.sort();
                for w in workflows {
                    if *w == cfg.active_workflow {
                        println!(" - {} (active)", w);
                    } else {
                        println!(" - {}", w);
                    }
                }
            }
            workflow_name => {
                let cfg = config::load_config()?;
                if let Some(wf) = cfg.workflows.get(workflow_name) {
                    println!("Launching workflow: {}", workflow_name);
                    launcher::launch_workflow(workflow_name, &wf.steps)?;
                } else {
                    eprintln!("Workflow '{}' not found in config.", workflow_name);
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}