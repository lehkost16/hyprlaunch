use crate::config::{load_config, save_config, Config, Workflow, WorkflowStep, StepType, StepCondition};
use crate::desktop::{scan_desktop_entries, DesktopEntry, find_desktop_file};
use crate::launcher::{launch_workflow, execute_step, get_launch_status};
use crate::health::validate_workflow_steps;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, BorderType, Clear, List, ListItem, ListState, Paragraph},
    text::{Line, Span},
    Terminal,
};
use std::io;

// Tokyo Night color palette constants
const TN_PRIMARY: Color = Color::Rgb(122, 162, 247); // soft blue (#7aa2f7)
const TN_ACTIVE: Color = Color::Rgb(125, 207, 255);  // cyan (#7dcfff)
const TN_INACTIVE: Color = Color::Rgb(86, 95, 137);  // slate blue (#565f89)
const TN_WARNING: Color = Color::Rgb(224, 175, 104); // yellow (#e0af68)
const TN_ERROR: Color = Color::Rgb(247, 118, 142);   // red (#f7768e)
const TN_SUCCESS: Color = Color::Rgb(158, 206, 106); // green (#9ece6a)
const TN_TEXT: Color = Color::Rgb(192, 202, 245);    // foreground (#c0caf5)
const TN_HIGHLIGHT: Color = Color::Rgb(187, 154, 247); // purple (#bb9af7)

enum ActivePane {
    Workflows,
    Steps,
}

enum AppState {
    Main,
    NewWorkflowSelector {
        selected_idx: usize,
    },
    NewWorkflowPrompt {
        input: String,
        cursor_pos: usize,
    },
    RenameWorkflowPrompt {
        old_name: String,
        input: String,
        cursor_pos: usize,
    },
    CloneWorkflowPrompt {
        old_name: String,
        input: String,
        cursor_pos: usize,
    },
    AddStepTypeSelector {
        selected_idx: usize,
    },
    AddAppSearch {
        search: String,
        selected_idx: usize,
        all_apps: Vec<DesktopEntry>,
        cursor_pos: usize,
    },
    AddLaunchCommandPrompt {
        input: String,
        cursor_pos: usize,
    },
    AddCustomCommandPrompt {
        input: String,
        cursor_pos: usize,
    },
    AddDispatchPrompt {
        input: String,
        cursor_pos: usize,
    },
    AddWaitPrompt {
        input: String,
        cursor_pos: usize,
    },
    AddNotifyPrompt {
        title_input: String,
        body_input: String,
        active_field: usize,
        cursor_pos: usize,
    },
    EditStepLaunch {
        step_idx: usize,
        desktop: String,
        workspace: String,
        silent: bool,
        monitor_list: Vec<String>,
        monitor_idx: usize,
        ws_true: String,
        ws_false: String,
        terminal: bool,
        active_field: usize,
        cursor_pos: usize,
    },
    EditStepScript {
        step_idx: usize,
        command: String,
        dir: String,
        terminal: bool,
        active_field: usize,
        cursor_pos: usize,
    },
    EditStepDispatch {
        step_idx: usize,
        command: String,
        cursor_pos: usize,
    },
    EditStepWait {
        step_idx: usize,
        ms: String,
        cursor_pos: usize,
    },
    EditStepNotify {
        step_idx: usize,
        title: String,
        body: String,
        active_field: usize,
        cursor_pos: usize,
    },
    EditStepConditions {
        step_idx: usize,
        if_battery_idx: usize, // 0 = Any, 1 = Battery Only, 2 = AC Power Only
        if_process: String,
        if_time: String,
        active_field: usize,
        cursor_pos: usize,
    },
    HealthCheckReport {
        reports: Vec<String>,
        scroll_idx: usize,
    },
}

fn get_preset_workflow(preset_name: &str) -> Option<Workflow> {
    match preset_name {
        "Deep Coding" => Some(Workflow {
            name: "Deep Coding".to_string(),
            description: Some("Workplace with editor and browser focus".to_string()),
            project_path: None,
            steps: vec![
                WorkflowStep {
                    step_type: StepType::Notify {
                        title: "Deep Coding".to_string(),
                        body: "Entering development space...".to_string(),
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
                WorkflowStep {
                    step_type: StepType::Wait { ms: 500 },
                    condition: None,
                },
                WorkflowStep {
                    step_type: StepType::Dispatch {
                        command: "workspace 2".to_string(),
                    },
                    condition: None,
                },
            ],
        }),
        "Gaming Mode" => Some(Workflow {
            name: "Gaming Mode".to_string(),
            description: Some("Optimize effects and boot Steam".to_string()),
            project_path: None,
            steps: vec![
                WorkflowStep {
                    step_type: StepType::Notify {
                        title: "Gaming Mode".to_string(),
                        body: "Disabling decorations and launching steam...".to_string(),
                    },
                    condition: None,
                },
                WorkflowStep {
                    step_type: StepType::RunScript {
                        command: "hyprctl keyword decoration:blur:enabled false".to_string(),
                        dir: None,
                        terminal: None,
                    },
                    condition: None,
                },
                WorkflowStep {
                    step_type: StepType::RunScript {
                        command: "hyprctl keyword decoration:drop_shadow false".to_string(),
                        dir: None,
                        terminal: None,
                    },
                    condition: None,
                },
                WorkflowStep {
                    step_type: StepType::Launch {
                        desktop: "steam.desktop".to_string(),
                        workspace: Some("4".to_string()),
                        silent: Some(false),
                        monitor_cond: None,
                        terminal: None,
                    },
                    condition: None,
                },
            ],
        }),
        "Content Creation" => Some(Workflow {
            name: "Content Creation".to_string(),
            description: Some("Studio environment for recording and notes".to_string()),
            project_path: None,
            steps: vec![
                WorkflowStep {
                    step_type: StepType::Notify {
                        title: "Media Studio".to_string(),
                        body: "Booting video studio...".to_string(),
                    },
                    condition: None,
                },
                WorkflowStep {
                    step_type: StepType::Launch {
                        desktop: "obsidian.desktop".to_string(),
                        workspace: Some("3".to_string()),
                        silent: Some(false),
                        monitor_cond: None,
                        terminal: None,
                    },
                    condition: None,
                },
                WorkflowStep {
                    step_type: StepType::Launch {
                        desktop: "obs.desktop".to_string(),
                        workspace: Some("3".to_string()),
                        silent: Some(true),
                        monitor_cond: None,
                        terminal: None,
                    },
                    condition: None,
                },
            ],
        }),
        "System Maintenance" => Some(Workflow {
            name: "System Maintenance".to_string(),
            description: Some("Runs system updates and cache cleaning".to_string()),
            project_path: None,
            steps: vec![
                WorkflowStep {
                    step_type: StepType::Notify {
                        title: "Maintenance".to_string(),
                        body: "Running system updates...".to_string(),
                    },
                    condition: None,
                },
                WorkflowStep {
                    step_type: StepType::Launch {
                        desktop: "kitty -e yay -Syu".to_string(),
                        workspace: None,
                        silent: Some(false),
                        monitor_cond: None,
                        terminal: None,
                    },
                    condition: None,
                },
                WorkflowStep {
                    step_type: StepType::RunScript {
                        command: "rm -rf ~/.cache/thumbnails/*".to_string(),
                        dir: None,
                        terminal: None,
                    },
                    condition: None,
                },
            ],
        }),
        _ => None,
    }
}

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_config()?;
    let all_desktop_apps = scan_desktop_entries();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_event_loop(&mut terminal, &mut config, all_desktop_apps);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("TUI Error: {:?}", err);
    }

    Ok(())
}

fn run_event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    config: &mut Config,
    all_desktop_apps: Vec<DesktopEntry>,
) -> io::Result<()> {
    let mut active_pane = ActivePane::Workflows;
    let mut state = AppState::Main;

    let mut workflow_list_state = ListState::default();
    let mut step_list_state = ListState::default();

    let mut workflows_list: Vec<String> = config.workflows.keys().cloned().collect();
    workflows_list.sort();
    if let Some(pos) = workflows_list.iter().position(|w| w == &config.active_workflow) {
        workflow_list_state.select(Some(pos));
    } else if !workflows_list.is_empty() {
        workflow_list_state.select(Some(0));
    }

    loop {
        let current_workflows: Vec<String> = {
            let mut w: Vec<String> = config.workflows.keys().cloned().collect();
            w.sort();
            w
        };

        let selected_workflow_name = workflow_list_state
            .selected()
            .and_then(|idx| current_workflows.get(idx))
            .cloned();

        let current_steps = selected_workflow_name
            .as_ref()
            .and_then(|name| config.workflows.get(name))
            .map(|w| w.steps.clone())
            .unwrap_or_default();

        if workflow_list_state.selected().is_some() && current_workflows.is_empty() {
            workflow_list_state.select(None);
        } else if workflow_list_state.selected().is_none() && !current_workflows.is_empty() {
            workflow_list_state.select(Some(0));
        }

        if step_list_state.selected().is_some() && current_steps.is_empty() {
            step_list_state.select(None);
        } else if step_list_state.selected().is_none() && !current_steps.is_empty() {
            step_list_state.select(Some(0));
        }

        let selected_w_name = selected_workflow_name.clone();
        let current_steps_clone = current_steps.clone();
        let cur_workflows = current_workflows.clone();

        // 1. Get running status details from background threads
        let active_status = {
            if let Ok(guard) = get_launch_status().lock() {
                guard.clone()
            } else {
                None
            }
        };

        let active_step_idx = if let Some(ref status) = active_status {
            if selected_w_name.as_ref() == Some(&status.workflow_name) {
                Some(status.current_step - 1)
            } else {
                None
            }
        } else {
            None
        };

        let mut cursor_screen_pos: Option<(u16, u16)> = None;

        terminal.draw(|f| {
            let size = f.size();

            // Calculate split direction dynamically for adaptive layouts
            let is_vertical_split = size.width < 100;

            // Main vertical layout
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(size);

            let (body_area, status_area) = if active_status.is_some() {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Min(1)])
                    .split(main_chunks[0]);
                (chunks[1], Some(chunks[0]))
            } else {
                (main_chunks[0], None)
            };

            // Render status bar
            if let Some(s_area) = status_area {
                if let Some(ref s) = active_status {
                    let pct = if s.total_steps > 0 { s.current_step * 100 / s.total_steps } else { 0 };
                    let status_msg = format!(" ⚡ RUNNING: {} | Step {}/{} | {} ({}%)", s.workflow_name, s.current_step, s.total_steps, s.step_description, pct);
                    let p_status = Paragraph::new(Line::from(vec![
                        Span::styled(status_msg, Style::default().fg(TN_TEXT).add_modifier(Modifier::BOLD))
                    ]))
                    .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(TN_WARNING)));
                    f.render_widget(p_status, s_area);
                }
            }

            // Split body area
            let body_chunks = if is_vertical_split {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                    .split(body_area)
            } else {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                    .split(body_area)
            };

            let left_border_style = match active_pane {
                ActivePane::Workflows => Style::default().fg(TN_ACTIVE).add_modifier(Modifier::BOLD),
                ActivePane::Steps => Style::default().fg(TN_INACTIVE),
            };
            let left_border_type = match active_pane {
                ActivePane::Workflows => BorderType::Thick,
                ActivePane::Steps => BorderType::Rounded,
            };

            let workflow_items: Vec<ListItem> = cur_workflows
                .iter()
                .map(|w| {
                    let active_indicator = if w == &config.active_workflow { "★" } else { " " };
                    ListItem::new(format!(" {} {}", active_indicator, w))
                })
                .collect();

            let workflow_list = List::new(workflow_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(left_border_type)
                        .title(" Workflows ")
                        .border_style(left_border_style),
                )
                .highlight_style(
                    Style::default()
                        .bg(TN_ACTIVE)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            f.render_stateful_widget(workflow_list, body_chunks[0], &mut workflow_list_state);

            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(body_chunks[1]);

            let right_border_style = match active_pane {
                ActivePane::Steps => Style::default().fg(TN_ACTIVE).add_modifier(Modifier::BOLD),
                ActivePane::Workflows => Style::default().fg(TN_INACTIVE),
            };
            let right_border_type = match active_pane {
                ActivePane::Steps => BorderType::Thick,
                ActivePane::Workflows => BorderType::Rounded,
            };

            let right_title = match &selected_w_name {
                Some(name) => format!(" Workflow: {} ", name),
                None => " Workflow Detail ".to_string(),
            };

            if current_steps_clone.is_empty() {
                let placeholder = Paragraph::new("

   No steps configured.
   Press 'a' to add your first step to this workflow.")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(right_border_type)
                            .title(right_title.as_str())
                            .border_style(right_border_style),
                    )
                    .style(Style::default().fg(TN_INACTIVE));
                f.render_widget(placeholder, right_chunks[0]);
            } else {
                let step_items: Vec<ListItem> = current_steps_clone
                    .iter()
                    .enumerate()
                    .map(|(i, step)| {
                        let step_num = i + 1;
                        let cond_prefix = if step.condition.is_some() { "[?] " } else { "" };
                        let is_running = active_step_idx == Some(i);

                        let prefix = if is_running { "▶ " } else { "" };
                        let running_style = if is_running { Style::default().fg(TN_WARNING).add_modifier(Modifier::BOLD) } else { Style::default() };

                        let list_item = match &step.step_type {
                            StepType::Launch { desktop, workspace, silent, monitor_cond, terminal } => {
                                let label = std::path::Path::new(desktop)
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or(desktop);
                                    
                                let mut tags = Vec::new();
                                if let Some(ws) = workspace {
                                    tags.push(format!("WS: {}", ws));
                                }
                                if let Some(cond) = monitor_cond {
                                    tags.push(format!("Cond: {}", cond));
                                }
                                if silent.unwrap_or(false) {
                                    tags.push("Silent".to_string());
                                }
                                if terminal.unwrap_or(false) {
                                    tags.push("Terminal".to_string());
                                }
                                
                                let tags_str = if tags.is_empty() {
                                    "".to_string()
                                } else {
                                    format!(" ({})", tags.join(", "))
                                };
                                
                                let exists = if desktop.ends_with(".desktop") {
                                    find_desktop_file(desktop).is_some()
                                } else {
                                    true
                                };
                                let (icon, base_style) = if exists {
                                    ("🚀 ", Style::default())
                                } else {
                                    ("⚠️ ", Style::default().fg(TN_WARNING))
                                };
                                
                                ListItem::new(format!(
                                    "{}{}{} {}. Launch: {}{}",
                                    cond_prefix, prefix, icon, step_num, label, tags_str
                                )).style(if is_running { running_style } else { base_style })
                            }
                            StepType::RunScript { command, dir, terminal } => {
                                let cmd_short = if command.len() > 35 {
                                    format!("{}...", &command[..32])
                                } else {
                                    command.clone()
                                };
                                let dir_str = dir.as_ref().map(|d| {
                                    let dir_name = std::path::Path::new(d)
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or(d);
                                    format!(" (in {})", dir_name)
                                }).unwrap_or_default();
                                let term_str = if terminal.unwrap_or(false) { " (Terminal)" } else { "" };
                                ListItem::new(format!(
                                    "{}{}⚙️  {}. Script: {}{}{}",
                                    cond_prefix, prefix, step_num, cmd_short, dir_str, term_str
                                )).style(if is_running { running_style } else { Style::default().fg(TN_PRIMARY) })
                            }
                            StepType::Wait { ms } => {
                                ListItem::new(format!(
                                    "{}{}⏳ {}. Wait: {} ms",
                                    cond_prefix, prefix, step_num, ms
                                )).style(if is_running { running_style } else { Style::default().fg(TN_WARNING) })
                            }
                            StepType::Notify { title, body } => {
                                let body_short = if body.len() > 30 {
                                    format!("{}...", &body[..27])
                                } else {
                                    body.clone()
                                };
                                ListItem::new(format!(
                                    "{}{}🔔 {}. Notify: \"{}\" - {}",
                                    cond_prefix, prefix, step_num, title, body_short
                                )).style(if is_running { running_style } else { Style::default().fg(TN_HIGHLIGHT) })
                            }
                            StepType::Dispatch { command } => {
                                ListItem::new(format!(
                                    "{}{}⚡ {}. Dispatch: {}",
                                    cond_prefix, prefix, step_num, command
                                )).style(if is_running { running_style } else { Style::default().fg(TN_SUCCESS) })
                            }
                        };
                        list_item
                    })
                    .collect();

                let step_list = List::new(step_items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_type(right_border_type)
                            .title(right_title.as_str())
                            .border_style(right_border_style),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(TN_ACTIVE)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");

                f.render_stateful_widget(step_list, right_chunks[0], &mut step_list_state);
            }

            let details_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Step Details ")
                .border_style(Style::default().fg(TN_INACTIVE));

            let mut details_lines = Vec::new();
            if let Some(idx) = step_list_state.selected() {
                if let Some(step) = current_steps_clone.get(idx) {
                    match &step.step_type {
                        StepType::Launch { desktop, workspace, silent, monitor_cond, terminal } => {
                            let is_desktop = desktop.ends_with(".desktop");
                            let status_text = if is_desktop {
                                if find_desktop_file(desktop).is_some() {
                                    "✔ Desktop Entry Found"
                                } else {
                                    "✘ Desktop Entry Not Found"
                                }
                            } else {
                                "✔ Custom Command / Binary"
                            };
                            let status_style = if is_desktop && find_desktop_file(desktop).is_none() {
                                Style::default().fg(TN_ERROR)
                            } else {
                                Style::default().fg(TN_SUCCESS)
                            };

                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Launch Application", Style::default().fg(TN_TEXT).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Target:  "),
                                Span::styled(desktop, Style::default().fg(TN_ACTIVE))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Config:  "),
                                Span::raw(format!(
                                    "Workspace: {} | Silent: {} | Monitor Condition: {} | Terminal: {}",
                                    workspace.as_deref().unwrap_or("Default"),
                                    if silent.unwrap_or(false) { "Yes" } else { "No" },
                                    monitor_cond.as_deref().unwrap_or("None"),
                                    if terminal.unwrap_or(false) { "Yes" } else { "No" }
                                ))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Status:  "),
                                Span::styled(status_text, status_style)
                            ]));
                        }
                        StepType::RunScript { command, dir, terminal } => {
                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Run Shell Script / Command", Style::default().fg(TN_TEXT).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Command: "),
                                Span::styled(command, Style::default().fg(TN_ACTIVE))
                            ]));
                            if let Some(d) = dir {
                                details_lines.push(Line::from(vec![
                                    Span::raw(" Dir:     "),
                                    Span::raw(d)
                                ]));
                            }
                            details_lines.push(Line::from(vec![
                                Span::raw(" Config:  "),
                                Span::raw(format!("Terminal: {}", if terminal.unwrap_or(false) { "Yes" } else { "No" }))
                            ]));
                        }
                        StepType::Wait { ms } => {
                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Wait / Sleep Delay", Style::default().fg(TN_TEXT).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Delay:   "),
                                Span::styled(format!("{} milliseconds", ms), Style::default().fg(TN_ACTIVE))
                            ]));
                        }
                        StepType::Notify { title, body } => {
                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Desktop Notification", Style::default().fg(TN_TEXT).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Title:   "),
                                Span::styled(title, Style::default().fg(TN_ACTIVE))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Body:    "),
                                Span::raw(body)
                            ]));
                        }
                        StepType::Dispatch { command } => {
                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Hyprland Dispatch Command", Style::default().fg(TN_TEXT).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Dispatch:"),
                                Span::styled(command, Style::default().fg(TN_ACTIVE))
                            ]));
                        }
                    }
                    
                    if let Some(ref cond) = step.condition {
                        let mut cond_tags = Vec::new();
                        if let Some(bat) = cond.if_battery {
                            cond_tags.push(format!("Power: {}", if bat { "Battery" } else { "AC" }));
                        }
                        if let Some(ref proc) = cond.if_process_not_running {
                            cond_tags.push(format!("ProcRunning != {}", proc));
                        }
                        if let Some(ref t) = cond.if_time {
                            cond_tags.push(format!("Time: {}", t));
                        }
                        details_lines.push(Line::from(vec![
                            Span::raw(" If Cond: "),
                            Span::styled(cond_tags.join(" | "), Style::default().fg(TN_WARNING))
                        ]));
                    }
                } else {
                    details_lines.push(Line::from(" No step selected."));
                }
            } else {
                details_lines.push(Line::from(" Select a step to view details."));
            };

            let details_paragraph = Paragraph::new(details_lines)
                .block(details_block);
            f.render_widget(details_paragraph, right_chunks[1]);

            // Highlighting keycaps in help bar
            let help_spans = match active_pane {
                ActivePane::Workflows => vec![
                    Span::styled(" [Enter] ", Style::default().fg(TN_ACTIVE).add_modifier(Modifier::BOLD)),
                    Span::raw("Launch  "),
                    Span::styled(" [Space] ", Style::default().fg(TN_ACTIVE).add_modifier(Modifier::BOLD)),
                    Span::raw("Active  "),
                    Span::styled(" [c] ", Style::default().fg(TN_HIGHLIGHT).add_modifier(Modifier::BOLD)),
                    Span::raw("Create  "),
                    Span::styled(" [r] ", Style::default().fg(TN_HIGHLIGHT).add_modifier(Modifier::BOLD)),
                    Span::raw("Rename  "),
                    Span::styled(" [y] ", Style::default().fg(TN_HIGHLIGHT).add_modifier(Modifier::BOLD)),
                    Span::raw("Clone  "),
                    Span::styled(" [d] ", Style::default().fg(TN_ERROR).add_modifier(Modifier::BOLD)),
                    Span::raw("Delete  "),
                    Span::styled(" [Tab] ", Style::default().fg(TN_PRIMARY).add_modifier(Modifier::BOLD)),
                    Span::raw("Edit Steps  "),
                    Span::styled(" [v] ", Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD)),
                    Span::raw("Health  "),
                    Span::styled(" [q] ", Style::default().fg(TN_ERROR).add_modifier(Modifier::BOLD)),
                    Span::raw("Quit"),
                ],
                ActivePane::Steps => vec![
                    Span::styled(" [Tab/Esc] ", Style::default().fg(TN_PRIMARY).add_modifier(Modifier::BOLD)),
                    Span::raw("Back  "),
                    Span::styled(" [Enter] ", Style::default().fg(TN_ACTIVE).add_modifier(Modifier::BOLD)),
                    Span::raw("Test  "),
                    Span::styled(" [a] ", Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD)),
                    Span::raw("Add  "),
                    Span::styled(" [e] ", Style::default().fg(TN_HIGHLIGHT).add_modifier(Modifier::BOLD)),
                    Span::raw("Edit  "),
                    Span::styled(" [c] ", Style::default().fg(TN_HIGHLIGHT).add_modifier(Modifier::BOLD)),
                    Span::raw("Cond  "),
                    Span::styled(" [d] ", Style::default().fg(TN_ERROR).add_modifier(Modifier::BOLD)),
                    Span::raw("Delete  "),
                    Span::styled(" [Ctrl+Up/Down] ", Style::default().fg(TN_WARNING).add_modifier(Modifier::BOLD)),
                    Span::raw("Move  "),
                    Span::styled(" [q] ", Style::default().fg(TN_ERROR).add_modifier(Modifier::BOLD)),
                    Span::raw("Quit"),
                ],
            };

            let help_paragraph = Paragraph::new(Line::from(help_spans))
                .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).title(" Operations "));
            f.render_widget(help_paragraph, main_chunks[1]);

            // Popups rendering
            match &state {
                AppState::NewWorkflowSelector { selected_idx } => {
                    let popup_area = centered_rect(50, 45, size);
                    f.render_widget(Clear, popup_area);
                    let options = vec![
                        "1. Create Blank Workflow",
                        "2. Preset: Deep Coding",
                        "3. Preset: Gaming Mode",
                        "4. Preset: Content Creation",
                        "5. Preset: System Maintenance",
                    ];
                    let items: Vec<ListItem> = options.iter().enumerate().map(|(i, opt)| {
                        let style = if i == *selected_idx {
                            Style::default().bg(TN_ACTIVE).fg(Color::Black).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(format!("  {}", opt)).style(style)
                    }).collect();

                    let list = List::new(items)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Select Workflow Template ")
                                .border_style(Style::default().fg(TN_WARNING))
                        );
                    f.render_widget(list, popup_area);
                }
                AppState::NewWorkflowPrompt { input, cursor_pos } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  Input: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Create New Workflow ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 9 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::RenameWorkflowPrompt { input, cursor_pos, .. } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  Rename: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Rename Workflow ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 10 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::CloneWorkflowPrompt { input, cursor_pos, .. } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  New Name: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Duplicate Workflow ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 12 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::AddStepTypeSelector { selected_idx } => {
                    let popup_area = centered_rect(50, 42, size);
                    f.render_widget(Clear, popup_area);
                    let options = vec![
                        "1. Launch Desktop Application",
                        "2. Launch Custom Window Command",
                        "3. Run Custom Command / Script in Background",
                        "4. Send Hyprctl Dispatch Command",
                        "5. Wait / Delay (milliseconds)",
                        "6. Send System Notification",
                    ];
                    let items: Vec<ListItem> = options.iter().enumerate().map(|(i, opt)| {
                        let style = if i == *selected_idx {
                            Style::default().bg(TN_ACTIVE).fg(Color::Black).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(format!("  {}", opt)).style(style)
                    }).collect();

                    let list = List::new(items)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Select Step Type to Add ")
                                .border_style(Style::default().fg(TN_WARNING))
                        );
                    f.render_widget(list, popup_area);
                }
                AppState::AddLaunchCommandPrompt { input, cursor_pos } => {
                    let popup_area = centered_rect(60, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  Command: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Add Custom Window Command ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 11 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::AddCustomCommandPrompt { input, cursor_pos } => {
                    let popup_area = centered_rect(60, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  Command: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Add Custom Command / Script ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 11 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::AddDispatchPrompt { input, cursor_pos } => {
                    let popup_area = centered_rect(60, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  Dispatch: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Add Dispatch Command ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 12 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::AddWaitPrompt { input, cursor_pos } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  Duration (ms): {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Add Wait Step ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 17 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::AddNotifyPrompt { title_input, body_input, active_field, cursor_pos } => {
                    let popup_area = centered_rect(60, 30, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .title(" Add Notification Step ")
                        .border_style(Style::default().fg(TN_WARNING));
                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(1)])
                        .split(inner);

                    let styles = [
                        if *active_field == 0 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 1 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                    ];

                    let p_title = Paragraph::new(format!(" {}", title_input))
                        .block(Block::default().borders(Borders::ALL).title(" Notification Title ").border_style(styles[0]));
                    let p_body = Paragraph::new(format!(" {}", body_input))
                        .block(Block::default().borders(Borders::ALL).title(" Notification Body ").border_style(styles[1]));

                    f.render_widget(p_title, chunks[0]);
                    f.render_widget(p_body, chunks[1]);

                    let active_rect = chunks[*active_field];
                    cursor_screen_pos = Some((active_rect.x + 2 + *cursor_pos as u16, active_rect.y + 1));
                }
                AppState::AddAppSearch { search, selected_idx, all_apps, cursor_pos } => {
                    let popup_area = centered_rect(70, 75, size);
                    f.render_widget(Clear, popup_area);

                    let main_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .title(" Add Desktop Application ")
                        .border_style(Style::default().fg(TN_WARNING));
                    let inner = main_block.inner(popup_area);
                    f.render_widget(main_block, popup_area);

                    let overlay_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Min(1)])
                        .split(inner);

                    let search_bar = Paragraph::new(format!(" {}", search))
                        .block(Block::default().borders(Borders::ALL).title(" Search Application Name ").border_style(Style::default().fg(TN_ACTIVE)));
                    f.render_widget(search_bar, overlay_chunks[0]);

                    cursor_screen_pos = Some((overlay_chunks[0].x + 2 + *cursor_pos as u16, overlay_chunks[0].y + 1));

                    let filtered: Vec<&DesktopEntry> = all_apps
                        .iter()
                        .filter(|entry| {
                            entry.name.to_lowercase().contains(&search.to_lowercase())
                                || entry
                                    .filename
                                    .to_lowercase()
                                    .contains(&search.to_lowercase())
                        })
                        .collect();

                    let items: Vec<ListItem> = filtered
                        .iter()
                        .map(|entry| {
                            let term_flag = if entry.terminal { " (Terminal)" } else { "" };
                            ListItem::new(format!("{}   [{}]{}", entry.name, entry.filename, term_flag))
                        })
                        .collect();

                    let mut list_state = ListState::default();
                    if !filtered.is_empty() {
                        let clamped_idx = (*selected_idx).min(filtered.len() - 1);
                        list_state.select(Some(clamped_idx));
                    }

                    let app_search_list = List::new(items)
                        .block(Block::default().borders(Borders::ALL).title(" Match Results "))
                        .highlight_style(
                            Style::default()
                                .bg(TN_SUCCESS)
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD),
                        )
                        .highlight_symbol("> ");

                    f.render_stateful_widget(app_search_list, overlay_chunks[1], &mut list_state);
                }
                AppState::EditStepLaunch {
                    desktop,
                    workspace,
                    silent,
                    monitor_list,
                    monitor_idx,
                    ws_true,
                    ws_false,
                    terminal: run_term,
                    active_field,
                    cursor_pos,
                    ..
                } => {
                    let popup_area = centered_rect(70, 78, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .title(" Edit Launch Step ")
                        .border_style(Style::default().fg(TN_WARNING));

                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3), // Desktop
                            Constraint::Length(3), // Workspace
                            Constraint::Length(3), // Monitor (Selectable)
                            Constraint::Length(3), // WS if Connected
                            Constraint::Length(3), // WS if Not Connected
                            Constraint::Length(3), // Silent
                            Constraint::Length(3), // Terminal
                            Constraint::Min(1)
                        ])
                        .split(inner);

                    let styles = [
                        if *active_field == 0 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 1 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 2 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 3 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 4 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 5 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 6 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                    ];

                    let p_desktop = Paragraph::new(format!(" {}", desktop))
                        .block(Block::default().borders(Borders::ALL).title(" Desktop File / Command ").border_style(styles[0]));
                    let p_ws = Paragraph::new(format!(" {}", workspace))
                        .block(Block::default().borders(Borders::ALL).title(" Target Workspace (optional) ").border_style(styles[1]));
                    
                    let monitor_val = &monitor_list[*monitor_idx];
                    let p_monitor = Paragraph::new(format!("  {} (Press Space or Left/Right to Cycle)", monitor_val))
                        .block(Block::default().borders(Borders::ALL).title(" Monitor Condition (Select connected monitor) ").border_style(styles[2]));
                        
                    let p_wstrue = Paragraph::new(format!(" {}", ws_true))
                        .block(Block::default().borders(Borders::ALL).title(" Workspace if Monitor Connected ").border_style(styles[3]));
                    let p_wsfalse = Paragraph::new(format!(" {}", ws_false))
                        .block(Block::default().borders(Borders::ALL).title(" Workspace if Monitor NOT Connected ").border_style(styles[4]));

                    let silent_val = if *silent { "[X] Enabled" } else { "[ ] Disabled" };
                    let p_silent = Paragraph::new(format!("  {} (Press Space to Toggle)", silent_val))
                        .block(Block::default().borders(Borders::ALL).title(" Silent Run ").border_style(styles[5]));

                    let terminal_val = if *run_term { "[X] Enabled" } else { "[ ] Disabled" };
                    let p_terminal = Paragraph::new(format!("  {} (Press Space to Toggle)", terminal_val))
                        .block(Block::default().borders(Borders::ALL).title(" Run in Terminal ").border_style(styles[6]));

                    f.render_widget(p_desktop, chunks[0]);
                    f.render_widget(p_ws, chunks[1]);
                    f.render_widget(p_monitor, chunks[2]);
                    f.render_widget(p_wstrue, chunks[3]);
                    f.render_widget(p_wsfalse, chunks[4]);
                    f.render_widget(p_silent, chunks[5]);
                    f.render_widget(p_terminal, chunks[6]);

                    let help = Paragraph::new("
  Tab/Arrow: Switch Fields | Space/Left/Right: Cycle Monitor | Space on Silent/Terminal: Toggle | Enter: Save | Esc: Cancel")
                        .style(Style::default().fg(TN_INACTIVE));
                    f.render_widget(help, chunks[7]);

                    // Determine hardware cursor position for text inputs
                    if *active_field != 2 && *active_field != 5 && *active_field != 6 {
                        let active_chunk = chunks[*active_field];
                        cursor_screen_pos = Some((active_chunk.x + 2 + *cursor_pos as u16, active_chunk.y + 1));
                    }
                }
                AppState::EditStepScript { command, dir, terminal: run_term, active_field, cursor_pos, .. } => {
                    let popup_area = centered_rect(70, 52, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .title(" Edit Script / Command Step ")
                        .border_style(Style::default().fg(TN_WARNING));

                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3), // Command
                            Constraint::Length(3), // Working Dir
                            Constraint::Length(3), // Terminal
                            Constraint::Min(1)
                        ])
                        .split(inner);

                    let styles = [
                        if *active_field == 0 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 1 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 2 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                    ];

                    let p_cmd = Paragraph::new(format!(" {}", command))
                        .block(Block::default().borders(Borders::ALL).title(" Command / Script ").border_style(styles[0]));
                    let p_dir = Paragraph::new(format!(" {}", dir))
                        .block(Block::default().borders(Borders::ALL).title(" Working Directory (optional) ").border_style(styles[1]));
                    let terminal_val = if *run_term { "[X] Enabled" } else { "[ ] Disabled" };
                    let p_terminal = Paragraph::new(format!("  {} (Press Space to Toggle)", terminal_val))
                        .block(Block::default().borders(Borders::ALL).title(" Run in Terminal ").border_style(styles[2]));

                    f.render_widget(p_cmd, chunks[0]);
                    f.render_widget(p_dir, chunks[1]);
                    f.render_widget(p_terminal, chunks[2]);

                    let help = Paragraph::new("
  Tab/Arrow: Switch Fields | Space on Terminal: Toggle | Enter: Save | Esc: Cancel")
                        .style(Style::default().fg(TN_INACTIVE));
                    f.render_widget(help, chunks[3]);

                    if *active_field != 2 {
                        let active_chunk = chunks[*active_field];
                        cursor_screen_pos = Some((active_chunk.x + 2 + *cursor_pos as u16, active_chunk.y + 1));
                    }
                }
                AppState::EditStepDispatch { command, cursor_pos, .. } => {
                    let popup_area = centered_rect(60, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  Dispatch: {}", command))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Edit Dispatch Step ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 12 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::EditStepWait { ms, cursor_pos, .. } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("
  Duration (ms): {}", ms))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Edit Wait Step ")
                                .border_style(Style::default().fg(TN_WARNING)),
                        );
                    f.render_widget(input_block, popup_area);
                    cursor_screen_pos = Some((popup_area.x + 17 + *cursor_pos as u16, popup_area.y + 2));
                }
                AppState::EditStepNotify { title, body, active_field, cursor_pos, .. } => {
                    let popup_area = centered_rect(60, 30, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .title(" Edit Notification Step ")
                        .border_style(Style::default().fg(TN_WARNING));
                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(1)])
                        .split(inner);

                    let styles = [
                        if *active_field == 0 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 1 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                    ];

                    let p_title = Paragraph::new(format!(" {}", title))
                        .block(Block::default().borders(Borders::ALL).title(" Notification Title ").border_style(styles[0]));
                    let p_body = Paragraph::new(format!(" {}", body))
                        .block(Block::default().borders(Borders::ALL).title(" Notification Body ").border_style(styles[1]));

                    f.render_widget(p_title, chunks[0]);
                    f.render_widget(p_body, chunks[1]);

                    let active_rect = chunks[*active_field];
                    cursor_screen_pos = Some((active_rect.x + 2 + *cursor_pos as u16, active_rect.y + 1));
                }
                AppState::EditStepConditions { if_battery_idx, if_process, if_time, active_field, cursor_pos, .. } => {
                    let popup_area = centered_rect(65, 38, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .title(" Edit Step Conditions ")
                        .border_style(Style::default().fg(TN_WARNING));
                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3), // Power
                            Constraint::Length(3), // Process running
                            Constraint::Length(3), // Time range
                            Constraint::Min(1)
                        ])
                        .split(inner);

                    let styles = [
                        if *active_field == 0 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 1 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                        if *active_field == 2 { Style::default().fg(TN_SUCCESS).add_modifier(Modifier::BOLD) } else { Style::default().fg(TN_INACTIVE) },
                    ];

                    let bat_val = match if_battery_idx {
                        1 => "Run Only on Battery Power",
                        2 => "Run Only on AC (Plugged-In) Power",
                        _ => "Run on Any Power Source (Default)",
                    };
                    let p_bat = Paragraph::new(format!("  {} (Press Space or Left/Right to Cycle)", bat_val))
                        .block(Block::default().borders(Borders::ALL).title(" Power Constraints ").border_style(styles[0]));

                    let p_proc = Paragraph::new(format!(" {}", if_process))
                        .block(Block::default().borders(Borders::ALL).title(" Skip step if this Process is already Running ").border_style(styles[1]));

                    let p_time = Paragraph::new(format!(" {}", if_time))
                        .block(Block::default().borders(Borders::ALL).title(" Time constraint (e.g. between 09:00-18:00) ").border_style(styles[2]));

                    f.render_widget(p_bat, chunks[0]);
                    f.render_widget(p_proc, chunks[1]);
                    f.render_widget(p_time, chunks[2]);

                    let help = Paragraph::new("
  Tab/Arrow: Switch Fields | Space/Left/Right (on Power): Cycle | Enter: Save | Esc: Cancel")
                        .style(Style::default().fg(TN_INACTIVE));
                    f.render_widget(help, chunks[3]);

                    if *active_field != 0 {
                        let active_chunk = chunks[*active_field];
                        cursor_screen_pos = Some((active_chunk.x + 2 + *cursor_pos as u16, active_chunk.y + 1));
                    }
                }
                AppState::HealthCheckReport { reports, scroll_idx } => {
                    let popup_area = centered_rect(80, 80, size);
                    f.render_widget(Clear, popup_area);

                    let list_items: Vec<ListItem> = reports
                        .iter()
                        .skip(*scroll_idx)
                        .enumerate()
                        .map(|(i, r)| {
                            let style = if r.contains("not found") || r.contains("does not exist") || r.contains("invalid") || r.contains("Error") {
                                Style::default().fg(TN_ERROR)
                            } else {
                                Style::default().fg(TN_WARNING)
                            };
                            ListItem::new(format!(" {}. {}", i + 1 + *scroll_idx, r)).style(style)
                        })
                        .collect();

                    let list = List::new(list_items)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Double)
                                .title(" Workflow Diagnostics Check ")
                                .border_style(Style::default().fg(TN_WARNING))
                        );
                    f.render_widget(list, popup_area);
                }
                _ => {}
            }
        })?;

        // 2. Set hardware terminal cursor dynamically
        if let Some((cx, cy)) = cursor_screen_pos {
            terminal.show_cursor()?;
            terminal.set_cursor(cx, cy)?;
        } else {
            terminal.hide_cursor()?;
        }

        // 3. Handle TUI inputs
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Release {
                    continue;
                }

                match &mut state {
                    AppState::Main => match active_pane {
                        ActivePane::Workflows => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                                active_pane = ActivePane::Steps;
                                if !current_steps.is_empty() {
                                    step_list_state.select(Some(0));
                                } else {
                                    step_list_state.select(None);
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if let Some(selected) = workflow_list_state.selected() {
                                    if selected > 0 {
                                        workflow_list_state.select(Some(selected - 1));
                                    }
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if let Some(selected) = workflow_list_state.selected() {
                                    if selected + 1 < current_workflows.len() {
                                        workflow_list_state.select(Some(selected + 1));
                                    }
                                }
                            }
                            KeyCode::Char(' ') => {
                                if let Some(w_name) = &selected_workflow_name {
                                    config.active_workflow = w_name.clone();
                                    let _ = save_config(config);
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(w_name) = &selected_workflow_name {
                                    config.active_workflow = w_name.clone();
                                    let _ = save_config(config);
                                    let steps_clone = current_steps.clone();
                                    let w_name_clone = w_name.clone();
                                    std::thread::spawn(move || {
                                        let _ = launch_workflow(&w_name_clone, &steps_clone);
                                    });
                                }
                            }
                            KeyCode::Char('c') => {
                                state = AppState::NewWorkflowSelector { selected_idx: 0 };
                            }
                            KeyCode::Char('r') => {
                                if let Some(w_name) = &selected_workflow_name {
                                    state = AppState::RenameWorkflowPrompt {
                                        old_name: w_name.clone(),
                                        input: w_name.clone(),
                                        cursor_pos: w_name.len(),
                                    };
                                }
                            }
                            KeyCode::Char('y') => {
                                if let Some(w_name) = &selected_workflow_name {
                                    let clone_name = format!("{}_copy", w_name);
                                    let clone_len = clone_name.len();
                                    state = AppState::CloneWorkflowPrompt {
                                        old_name: w_name.clone(),
                                        input: clone_name,
                                        cursor_pos: clone_len,
                                    };
                                }
                            }
                            KeyCode::Char('d') => {
                                if let Some(w_name) = &selected_workflow_name {
                                    if current_workflows.len() > 1 {
                                        config.workflows.remove(w_name);
                                        if config.active_workflow == *w_name {
                                            let mut remaining: Vec<String> = config.workflows.keys().cloned().collect();
                                            remaining.sort();
                                            config.active_workflow = remaining[0].clone();
                                        }
                                        let _ = save_config(config);
                                        
                                        // Adjust selection index
                                        let mut w_list: Vec<String> = config.workflows.keys().cloned().collect();
                                        w_list.sort();
                                        workflows_list = w_list;
                                        if let Some(pos) = workflows_list.iter().position(|w| w == &config.active_workflow) {
                                            workflow_list_state.select(Some(pos));
                                        } else {
                                            workflow_list_state.select(Some(0));
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('v') => {
                                if let Some(w_name) = &selected_workflow_name {
                                    if let Some(wf) = config.workflows.get(w_name) {
                                        let reports = validate_workflow_steps(&wf.steps);
                                        if reports.is_empty() {
                                            state = AppState::HealthCheckReport {
                                                reports: vec!["✔ All checks passed. No diagnostics issues found.".to_string()],
                                                scroll_idx: 0,
                                            };
                                        } else {
                                            state = AppState::HealthCheckReport {
                                                reports,
                                                scroll_idx: 0,
                                            };
                                        }
                                    }
                                }
                            }
                            _ => {}
                        },
                        ActivePane::Steps => match key.code {
                            KeyCode::Tab | KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                                active_pane = ActivePane::Workflows;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) || key.modifiers.contains(KeyModifiers::CONTROL) {
                                    if let (Some(w_name), Some(idx)) = (&selected_workflow_name, step_list_state.selected()) {
                                        if idx > 0 {
                                            if let Some(wf) = config.workflows.get_mut(w_name) {
                                                wf.steps.swap(idx, idx - 1);
                                                step_list_state.select(Some(idx - 1));
                                                let _ = save_config(config);
                                            }
                                        }
                                    }
                                } else if let Some(selected) = step_list_state.selected() {
                                    if selected > 0 {
                                        step_list_state.select(Some(selected - 1));
                                    }
                                }
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                if key.modifiers.contains(KeyModifiers::SHIFT) || key.modifiers.contains(KeyModifiers::CONTROL) {
                                    if let (Some(w_name), Some(idx)) = (&selected_workflow_name, step_list_state.selected()) {
                                        if idx + 1 < current_steps.len() {
                                            if let Some(wf) = config.workflows.get_mut(w_name) {
                                                wf.steps.swap(idx, idx + 1);
                                                step_list_state.select(Some(idx + 1));
                                                let _ = save_config(config);
                                            }
                                        }
                                    }
                                } else if let Some(selected) = step_list_state.selected() {
                                    if selected + 1 < current_steps.len() {
                                        step_list_state.select(Some(selected + 1));
                                    }
                                }
                            }
                            KeyCode::Char('a') => {
                                state = AppState::AddStepTypeSelector {
                                    selected_idx: 0,
                                };
                            }
                            KeyCode::Char('e') => {
                                if let Some(idx) = step_list_state.selected() {
                                    if let Some(step) = current_steps.get(idx) {
                                        match &step.step_type {
                                            StepType::Launch { desktop, workspace, silent, monitor_cond, terminal } => {
                                                let active_monitors = crate::launcher::get_active_monitors();
                                                let mut monitor_list = vec!["None".to_string()];
                                                for m in active_monitors {
                                                    if !monitor_list.contains(&m) {
                                                        monitor_list.push(m);
                                                    }
                                                }
                                                let mut monitor_idx = 0;
                                                let mut ws_true = String::new();
                                                let mut ws_false = String::new();
                                                if let Some(ref cond) = monitor_cond {
                                                    if let Some(q_pos) = cond.find('?') {
                                                        let m_name = cond[..q_pos].trim().to_string();
                                                        let remainder = &cond[q_pos + 1..];
                                                        if let Some(colon_pos) = remainder.find(':') {
                                                            ws_true = remainder[..colon_pos].trim().to_string();
                                                            ws_false = remainder[colon_pos + 1..].trim().to_string();
                                                            if let Some(pos) = monitor_list.iter().position(|x| x == &m_name) {
                                                                monitor_idx = pos;
                                                            } else {
                                                                monitor_list.push(m_name);
                                                                monitor_idx = monitor_list.len() - 1;
                                                            }
                                                        }
                                                    }
                                                }
                                                let desktop_len = desktop.len();
                                                state = AppState::EditStepLaunch {
                                                    step_idx: idx,
                                                    desktop: desktop.clone(),
                                                    workspace: workspace.clone().unwrap_or_default(),
                                                    silent: silent.unwrap_or(false),
                                                    monitor_list,
                                                    monitor_idx,
                                                    ws_true,
                                                    ws_false,
                                                    terminal: terminal.unwrap_or(false),
                                                    active_field: 0,
                                                    cursor_pos: desktop_len,
                                                };
                                            }
                                            StepType::RunScript { command, dir, terminal } => {
                                                let cmd_len = command.len();
                                                state = AppState::EditStepScript {
                                                    step_idx: idx,
                                                    command: command.clone(),
                                                    dir: dir.clone().unwrap_or_default(),
                                                    terminal: terminal.unwrap_or(false),
                                                    active_field: 0,
                                                    cursor_pos: cmd_len,
                                                };
                                            }
                                            StepType::Dispatch { command } => {
                                                let cmd_len = command.len();
                                                state = AppState::EditStepDispatch {
                                                    step_idx: idx,
                                                    command: command.clone(),
                                                    cursor_pos: cmd_len,
                                                };
                                            }
                                            StepType::Wait { ms } => {
                                                let ms_str = ms.to_string();
                                                let ms_len = ms_str.len();
                                                state = AppState::EditStepWait {
                                                    step_idx: idx,
                                                    ms: ms_str,
                                                    cursor_pos: ms_len,
                                                };
                                            }
                                            StepType::Notify { title, body } => {
                                                let title_len = title.len();
                                                state = AppState::EditStepNotify {
                                                    step_idx: idx,
                                                    title: title.clone(),
                                                    body: body.clone(),
                                                    active_field: 0,
                                                    cursor_pos: title_len,
                                                };
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('c') => {
                                if let Some(idx) = step_list_state.selected() {
                                    if let Some(step) = current_steps.get(idx) {
                                        let (bat_idx, proc_str, time_str) = if let Some(ref cond) = step.condition {
                                            let b = match cond.if_battery {
                                                Some(true) => 1,
                                                Some(false) => 2,
                                                None => 0,
                                            };
                                            (b, cond.if_process_not_running.clone().unwrap_or_default(), cond.if_time.clone().unwrap_or_default())
                                        } else {
                                            (0, String::new(), String::new())
                                        };
                                        state = AppState::EditStepConditions {
                                            step_idx: idx,
                                            if_battery_idx: bat_idx,
                                            if_process: proc_str,
                                            if_time: time_str,
                                            active_field: 0,
                                            cursor_pos: 0,
                                        };
                                    }
                                }
                            }
                            KeyCode::Char('d') => {
                                if let Some(idx) = step_list_state.selected() {
                                    if let Some(w_name) = &selected_workflow_name {
                                        let steps_empty = if let Some(wf) = config.workflows.get_mut(w_name) {
                                            wf.steps.remove(idx);
                                            let empty = wf.steps.is_empty();
                                            if !empty {
                                                step_list_state.select(Some(idx.min(wf.steps.len() - 1)));
                                            }
                                            empty
                                        } else {
                                            false
                                        };

                                        if steps_empty {
                                            active_pane = ActivePane::Workflows;
                                        }
                                        let _ = save_config(config);
                                    }
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(idx) = step_list_state.selected() {
                                    let step = &current_steps[idx];
                                    let step_clone = step.clone();
                                    std::thread::spawn(move || {
                                        let _ = execute_step(&step_clone);
                                    });
                                }
                            }
                            KeyCode::Char('q') => return Ok(()),
                            _ => {}
                        },
                    },
                    AppState::NewWorkflowSelector { selected_idx } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if *selected_idx > 0 {
                                *selected_idx -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if *selected_idx < 4 {
                                *selected_idx += 1;
                            }
                        }
                        KeyCode::Enter => {
                            match selected_idx {
                                0 => {
                                    state = AppState::NewWorkflowPrompt {
                                        input: String::new(),
                                        cursor_pos: 0,
                                    };
                                }
                                idx => {
                                    let presets = vec!["Deep Coding", "Gaming Mode", "Content Creation", "System Maintenance"];
                                    let preset_name = presets[*idx - 1];
                                    if let Some(wf) = get_preset_workflow(preset_name) {
                                        config.workflows.insert(wf.name.clone(), wf);
                                        let _ = save_config(config);
                                        
                                        // Adjust selection index
                                        let mut w_list: Vec<String> = config.workflows.keys().cloned().collect();
                                        w_list.sort();
                                        workflows_list = w_list;
                                        if let Some(pos) = workflows_list.iter().position(|w| w == preset_name) {
                                            workflow_list_state.select(Some(pos));
                                        }
                                    }
                                    state = AppState::Main;
                                }
                            }
                        }
                        _ => {}
                    },
                    AppState::NewWorkflowPrompt { input, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < input.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                input.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < input.len() {
                                input.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            input.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() {
                                let new_wf = Workflow {
                                    name: trimmed.clone(),
                                    description: None,
                                    project_path: None,
                                    steps: Vec::new(),
                                };
                                config.workflows.insert(trimmed.clone(), new_wf);
                                let _ = save_config(config);
                                
                                // Adjust selection index
                                let mut w_list: Vec<String> = config.workflows.keys().cloned().collect();
                                w_list.sort();
                                workflows_list = w_list;
                                if let Some(pos) = workflows_list.iter().position(|w| w == &trimmed) {
                                    workflow_list_state.select(Some(pos));
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::RenameWorkflowPrompt { old_name, input, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < input.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                input.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < input.len() {
                                input.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            input.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() && trimmed != *old_name {
                                if let Some(wf) = config.workflows.remove(old_name) {
                                    let mut new_wf = wf;
                                    new_wf.name = trimmed.clone();
                                    config.workflows.insert(trimmed.clone(), new_wf);
                                    if config.active_workflow == *old_name {
                                        config.active_workflow = trimmed.clone();
                                    }
                                    let _ = save_config(config);
                                    
                                    // Adjust selection index
                                    let mut w_list: Vec<String> = config.workflows.keys().cloned().collect();
                                    w_list.sort();
                                    workflows_list = w_list;
                                    if let Some(pos) = workflows_list.iter().position(|w| w == &trimmed) {
                                        workflow_list_state.select(Some(pos));
                                    }
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::CloneWorkflowPrompt { old_name, input, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < input.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                input.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < input.len() {
                                input.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            input.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() && trimmed != *old_name {
                                if let Some(wf) = config.workflows.get(old_name) {
                                    let mut clone_wf = wf.clone();
                                    clone_wf.name = trimmed.clone();
                                    config.workflows.insert(trimmed.clone(), clone_wf);
                                    let _ = save_config(config);
                                    
                                    // Adjust selection index
                                    let mut w_list: Vec<String> = config.workflows.keys().cloned().collect();
                                    w_list.sort();
                                    workflows_list = w_list;
                                    if let Some(pos) = workflows_list.iter().position(|w| w == &trimmed) {
                                        workflow_list_state.select(Some(pos));
                                    }
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::AddStepTypeSelector { selected_idx } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if *selected_idx > 0 {
                                *selected_idx -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if *selected_idx < 5 {
                                *selected_idx += 1;
                            }
                        }
                        KeyCode::Enter => {
                            match selected_idx {
                                0 => {
                                    state = AppState::AddAppSearch {
                                        search: String::new(),
                                        selected_idx: 0,
                                        all_apps: all_desktop_apps.clone(),
                                        cursor_pos: 0,
                                    };
                                }
                                1 => {
                                    state = AppState::AddLaunchCommandPrompt {
                                        input: String::new(),
                                        cursor_pos: 0,
                                    };
                                }
                                2 => {
                                    state = AppState::AddCustomCommandPrompt {
                                        input: String::new(),
                                        cursor_pos: 0,
                                    };
                                }
                                3 => {
                                    state = AppState::AddDispatchPrompt {
                                        input: String::new(),
                                        cursor_pos: 0,
                                    };
                                }
                                4 => {
                                    state = AppState::AddWaitPrompt {
                                        input: String::new(),
                                        cursor_pos: 0,
                                    };
                                }
                                5 => {
                                    state = AppState::AddNotifyPrompt {
                                        title_input: String::new(),
                                        body_input: String::new(),
                                        active_field: 0,
                                        cursor_pos: 0,
                                    };
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    },
                    AppState::AddAppSearch { search, selected_idx, all_apps, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < search.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                search.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                                *selected_idx = 0;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < search.len() {
                                search.remove(*cursor_pos);
                                *selected_idx = 0;
                            }
                        }
                        KeyCode::Char(c) => {
                            search.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                            *selected_idx = 0;
                        }
                        KeyCode::Up => {
                            if *selected_idx > 0 {
                                *selected_idx -= 1;
                            }
                        }
                        KeyCode::Down => {
                            *selected_idx += 1;
                        }
                        KeyCode::Enter => {
                            let filtered: Vec<&DesktopEntry> = all_apps
                                .iter()
                                .filter(|entry| {
                                    entry.name.to_lowercase().contains(&search.to_lowercase())
                                        || entry
                                            .filename
                                            .to_lowercase()
                                            .contains(&search.to_lowercase())
                                })
                                .collect();

                            if !filtered.is_empty() {
                                let clamped_idx = (*selected_idx).min(filtered.len() - 1);
                                let selected_app = filtered[clamped_idx];

                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep {
                                            step_type: StepType::Launch {
                                                desktop: selected_app.filename.clone(),
                                                workspace: None,
                                                silent: Some(false),
                                                monitor_cond: None,
                                                terminal: None,
                                            },
                                            condition: None,
                                        });
                                        wf.steps.len()
                                    } else {
                                        1
                                    };

                                    let _ = save_config(config);

                                    active_pane = ActivePane::Steps;
                                    step_list_state.select(Some(new_len - 1));
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::AddLaunchCommandPrompt { input, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < input.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                input.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < input.len() {
                                input.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            input.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep {
                                            step_type: StepType::Launch {
                                                desktop: trimmed,
                                                workspace: None,
                                                silent: Some(false),
                                                monitor_cond: None,
                                                terminal: None,
                                            },
                                            condition: None,
                                        });
                                        wf.steps.len()
                                    } else {
                                        1
                                    };
                                    let _ = save_config(config);
                                    active_pane = ActivePane::Steps;
                                    step_list_state.select(Some(new_len - 1));
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::AddCustomCommandPrompt { input, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < input.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                input.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < input.len() {
                                input.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            input.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep {
                                            step_type: StepType::RunScript {
                                                command: trimmed,
                                                dir: None,
                                                terminal: None,
                                            },
                                            condition: None,
                                        });
                                        wf.steps.len()
                                    } else {
                                        1
                                    };
                                    let _ = save_config(config);
                                    active_pane = ActivePane::Steps;
                                    step_list_state.select(Some(new_len - 1));
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::AddDispatchPrompt { input, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < input.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                input.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < input.len() {
                                input.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            input.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep {
                                            step_type: StepType::Dispatch {
                                                command: trimmed,
                                            },
                                            condition: None,
                                        });
                                        wf.steps.len()
                                    } else {
                                        1
                                    };
                                    let _ = save_config(config);
                                    active_pane = ActivePane::Steps;
                                    step_list_state.select(Some(new_len - 1));
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::AddWaitPrompt { input, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < input.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                input.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < input.len() {
                                input.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            if c.is_numeric() {
                                input.insert(*cursor_pos, c);
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim();
                            if let Ok(ms_val) = trimmed.parse::<u64>() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep {
                                            step_type: StepType::Wait {
                                                ms: ms_val,
                                            },
                                            condition: None,
                                        });
                                        wf.steps.len()
                                    } else {
                                        1
                                    };
                                    let _ = save_config(config);
                                    active_pane = ActivePane::Steps;
                                    step_list_state.select(Some(new_len - 1));
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::AddNotifyPrompt { title_input, body_input, active_field, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            *active_field = (*active_field + 1) % 2;
                            *cursor_pos = if *active_field == 0 { title_input.len() } else { body_input.len() };
                        }
                        KeyCode::Up => {
                            *active_field = (*active_field + 1) % 2;
                            *cursor_pos = if *active_field == 0 { title_input.len() } else { body_input.len() };
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            let limit = if *active_field == 0 { title_input.len() } else { body_input.len() };
                            if *cursor_pos < limit {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                if *active_field == 0 {
                                    title_input.remove(*cursor_pos - 1);
                                } else {
                                    body_input.remove(*cursor_pos - 1);
                                }
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *active_field == 0 {
                                if *cursor_pos < title_input.len() {
                                    title_input.remove(*cursor_pos);
                                }
                            } else {
                                if *cursor_pos < body_input.len() {
                                    body_input.remove(*cursor_pos);
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            if *active_field == 0 {
                                title_input.insert(*cursor_pos, c);
                            } else {
                                body_input.insert(*cursor_pos, c);
                            }
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            let title = title_input.trim().to_string();
                            let body = body_input.trim().to_string();
                            if !title.is_empty() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep {
                                            step_type: StepType::Notify {
                                                title,
                                                body,
                                            },
                                            condition: None,
                                        });
                                        wf.steps.len()
                                    } else {
                                        1
                                    };
                                    let _ = save_config(config);
                                    active_pane = ActivePane::Steps;
                                    step_list_state.select(Some(new_len - 1));
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepLaunch { step_idx, desktop, workspace, silent, monitor_list, monitor_idx, ws_true, ws_false, terminal: run_term, active_field, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            *active_field = (*active_field + 1) % 7;
                            *cursor_pos = match *active_field {
                                0 => desktop.len(),
                                1 => workspace.len(),
                                3 => ws_true.len(),
                                4 => ws_false.len(),
                                _ => 0,
                            };
                        }
                        KeyCode::Up => {
                            *active_field = (*active_field + 6) % 7;
                            *cursor_pos = match *active_field {
                                0 => desktop.len(),
                                1 => workspace.len(),
                                3 => ws_true.len(),
                                4 => ws_false.len(),
                                _ => 0,
                            };
                        }
                        KeyCode::Char(' ') => {
                            match active_field {
                                2 => { *monitor_idx = (*monitor_idx + 1) % monitor_list.len(); }
                                5 => { *silent = !*silent; }
                                6 => { *run_term = !*run_term; }
                                0 => { desktop.insert(*cursor_pos, ' '); *cursor_pos += 1; }
                                1 => { workspace.insert(*cursor_pos, ' '); *cursor_pos += 1; }
                                3 => { ws_true.insert(*cursor_pos, ' '); *cursor_pos += 1; }
                                4 => { ws_false.insert(*cursor_pos, ' '); *cursor_pos += 1; }
                                _ => {}
                            }
                        }
                        KeyCode::Right => {
                            if *active_field == 2 {
                                *monitor_idx = (*monitor_idx + 1) % monitor_list.len();
                            } else if *active_field != 5 && *active_field != 6 {
                                let limit = match *active_field {
                                    0 => desktop.len(),
                                    1 => workspace.len(),
                                    3 => ws_true.len(),
                                    4 => ws_false.len(),
                                    _ => 0,
                                };
                                if *cursor_pos < limit {
                                    *cursor_pos += 1;
                                }
                            }
                        }
                        KeyCode::Left => {
                            if *active_field == 2 {
                                *monitor_idx = (*monitor_idx + monitor_list.len() - 1) % monitor_list.len();
                            } else if *active_field != 5 && *active_field != 6 {
                                if *cursor_pos > 0 {
                                    *cursor_pos -= 1;
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                match active_field {
                                    0 => { desktop.remove(*cursor_pos - 1); }
                                    1 => { workspace.remove(*cursor_pos - 1); }
                                    3 => { ws_true.remove(*cursor_pos - 1); }
                                    4 => { ws_false.remove(*cursor_pos - 1); }
                                    _ => {}
                                }
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            match active_field {
                                0 => { if *cursor_pos < desktop.len() { desktop.remove(*cursor_pos); } }
                                1 => { if *cursor_pos < workspace.len() { workspace.remove(*cursor_pos); } }
                                3 => { if *cursor_pos < ws_true.len() { ws_true.remove(*cursor_pos); } }
                                4 => { if *cursor_pos < ws_false.len() { ws_false.remove(*cursor_pos); } }
                                _ => {}
                            }
                        }
                        KeyCode::Char(c) => {
                            match active_field {
                                0 => { desktop.insert(*cursor_pos, c); *cursor_pos += 1; }
                                1 => { workspace.insert(*cursor_pos, c); *cursor_pos += 1; }
                                3 => { ws_true.insert(*cursor_pos, c); *cursor_pos += 1; }
                                4 => { ws_false.insert(*cursor_pos, c); *cursor_pos += 1; }
                                _ => {}
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(w_name) = &selected_workflow_name {
                                if let Some(wf) = config.workflows.get_mut(w_name) {
                                    let ws = if workspace.trim().is_empty() { None } else { Some(workspace.trim().to_string()) };
                                    let mc = if monitor_list[*monitor_idx] == "None" {
                                        None
                                    } else {
                                        Some(format!("{}?{}:{}", monitor_list[*monitor_idx], ws_true.trim(), ws_false.trim()))
                                    };
                                    
                                    // Preserve condition
                                    let old_cond = wf.steps[*step_idx].condition.clone();
                                    wf.steps[*step_idx] = WorkflowStep {
                                        step_type: StepType::Launch {
                                            desktop: desktop.trim().to_string(),
                                            workspace: ws,
                                            silent: Some(*silent),
                                            monitor_cond: mc,
                                            terminal: Some(*run_term),
                                        },
                                        condition: old_cond,
                                    };
                                    let _ = save_config(config);
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepScript { step_idx, command, dir, terminal: run_term, active_field, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            *active_field = (*active_field + 1) % 3;
                            *cursor_pos = match *active_field {
                                0 => command.len(),
                                1 => dir.len(),
                                _ => 0,
                            };
                        }
                        KeyCode::Up => {
                            *active_field = (*active_field + 2) % 3;
                            *cursor_pos = match *active_field {
                                0 => command.len(),
                                1 => dir.len(),
                                _ => 0,
                            };
                        }
                        KeyCode::Char(' ') => {
                            match active_field {
                                2 => { *run_term = !*run_term; }
                                0 => { command.insert(*cursor_pos, ' '); *cursor_pos += 1; }
                                1 => { dir.insert(*cursor_pos, ' '); *cursor_pos += 1; }
                                _ => {}
                            }
                        }
                        KeyCode::Right => {
                            if *active_field != 2 {
                                let limit = if *active_field == 0 { command.len() } else { dir.len() };
                                if *cursor_pos < limit {
                                    *cursor_pos += 1;
                                }
                            }
                        }
                        KeyCode::Left => {
                            if *active_field != 2 {
                                if *cursor_pos > 0 {
                                    *cursor_pos -= 1;
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                match active_field {
                                    0 => { command.remove(*cursor_pos - 1); }
                                    1 => { dir.remove(*cursor_pos - 1); }
                                    _ => {}
                                }
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            match active_field {
                                0 => { if *cursor_pos < command.len() { command.remove(*cursor_pos); } }
                                1 => { if *cursor_pos < dir.len() { dir.remove(*cursor_pos); } }
                                _ => {}
                            }
                        }
                        KeyCode::Char(c) => {
                            match active_field {
                                0 => { command.insert(*cursor_pos, c); *cursor_pos += 1; }
                                1 => { dir.insert(*cursor_pos, c); *cursor_pos += 1; }
                                _ => {}
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(w_name) = &selected_workflow_name {
                                if let Some(wf) = config.workflows.get_mut(w_name) {
                                    let d = if dir.trim().is_empty() { None } else { Some(dir.trim().to_string()) };
                                    
                                    let old_cond = wf.steps[*step_idx].condition.clone();
                                    wf.steps[*step_idx] = WorkflowStep {
                                        step_type: StepType::RunScript {
                                            command: command.trim().to_string(),
                                            dir: d,
                                            terminal: Some(*run_term),
                                        },
                                        condition: old_cond,
                                    };
                                    let _ = save_config(config);
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepDispatch { step_idx, command, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < command.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                command.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < command.len() {
                                command.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            command.insert(*cursor_pos, c);
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            if let Some(w_name) = &selected_workflow_name {
                                if let Some(wf) = config.workflows.get_mut(w_name) {
                                    let old_cond = wf.steps[*step_idx].condition.clone();
                                    wf.steps[*step_idx] = WorkflowStep {
                                        step_type: StepType::Dispatch {
                                            command: command.trim().to_string(),
                                        },
                                        condition: old_cond,
                                    };
                                    let _ = save_config(config);
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepWait { step_idx, ms, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            if *cursor_pos < ms.len() {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                ms.remove(*cursor_pos - 1);
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *cursor_pos < ms.len() {
                                ms.remove(*cursor_pos);
                            }
                        }
                        KeyCode::Char(c) => {
                            if c.is_numeric() {
                                ms.insert(*cursor_pos, c);
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Enter => {
                            let trimmed = ms.trim();
                            if let Ok(ms_val) = trimmed.parse::<u64>() {
                                if let Some(w_name) = &selected_workflow_name {
                                    if let Some(wf) = config.workflows.get_mut(w_name) {
                                        let old_cond = wf.steps[*step_idx].condition.clone();
                                        wf.steps[*step_idx] = WorkflowStep {
                                            step_type: StepType::Wait {
                                                ms: ms_val,
                                            },
                                            condition: old_cond,
                                        };
                                        let _ = save_config(config);
                                    }
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepNotify { step_idx, title, body, active_field, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            *active_field = (*active_field + 1) % 2;
                            *cursor_pos = if *active_field == 0 { title.len() } else { body.len() };
                        }
                        KeyCode::Up => {
                            *active_field = (*active_field + 1) % 2;
                            *cursor_pos = if *active_field == 0 { title.len() } else { body.len() };
                        }
                        KeyCode::Left => {
                            if *cursor_pos > 0 {
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Right => {
                            let limit = if *active_field == 0 { title.len() } else { body.len() };
                            if *cursor_pos < limit {
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                if *active_field == 0 {
                                    title.remove(*cursor_pos - 1);
                                } else {
                                    body.remove(*cursor_pos - 1);
                                }
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *active_field == 0 {
                                if *cursor_pos < title.len() {
                                    title.remove(*cursor_pos);
                                }
                            } else {
                                if *cursor_pos < body.len() {
                                    body.remove(*cursor_pos);
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            if *active_field == 0 {
                                title.insert(*cursor_pos, c);
                            } else {
                                body.insert(*cursor_pos, c);
                            }
                            *cursor_pos += 1;
                        }
                        KeyCode::Enter => {
                            if let Some(w_name) = &selected_workflow_name {
                                if let Some(wf) = config.workflows.get_mut(w_name) {
                                    let old_cond = wf.steps[*step_idx].condition.clone();
                                    wf.steps[*step_idx] = WorkflowStep {
                                        step_type: StepType::Notify {
                                            title: title.trim().to_string(),
                                            body: body.trim().to_string(),
                                        },
                                        condition: old_cond,
                                    };
                                    let _ = save_config(config);
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepConditions { step_idx, if_battery_idx, if_process, if_time, active_field, cursor_pos } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            *active_field = (*active_field + 1) % 3;
                            *cursor_pos = match *active_field {
                                1 => if_process.len(),
                                2 => if_time.len(),
                                _ => 0,
                            };
                        }
                        KeyCode::Up => {
                            *active_field = (*active_field + 2) % 3;
                            *cursor_pos = match *active_field {
                                1 => if_process.len(),
                                2 => if_time.len(),
                                _ => 0,
                            };
                        }
                        KeyCode::Char(' ') => {
                            match active_field {
                                0 => { *if_battery_idx = (*if_battery_idx + 1) % 3; }
                                1 => { if_process.insert(*cursor_pos, ' '); *cursor_pos += 1; }
                                2 => { if_time.insert(*cursor_pos, ' '); *cursor_pos += 1; }
                                _ => {}
                            }
                        }
                        KeyCode::Right => {
                            if *active_field == 0 {
                                *if_battery_idx = (*if_battery_idx + 1) % 3;
                            } else {
                                let limit = if *active_field == 1 { if_process.len() } else { if_time.len() };
                                if *cursor_pos < limit {
                                    *cursor_pos += 1;
                                }
                            }
                        }
                        KeyCode::Left => {
                            if *active_field == 0 {
                                *if_battery_idx = (*if_battery_idx + 2) % 3;
                            } else {
                                if *cursor_pos > 0 {
                                    *cursor_pos -= 1;
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if *cursor_pos > 0 {
                                if *active_field == 1 {
                                    if_process.remove(*cursor_pos - 1);
                                } else if *active_field == 2 {
                                    if_time.remove(*cursor_pos - 1);
                                }
                                *cursor_pos -= 1;
                            }
                        }
                        KeyCode::Delete => {
                            if *active_field == 1 {
                                if *cursor_pos < if_process.len() {
                                    if_process.remove(*cursor_pos);
                                }
                            } else if *active_field == 2 {
                                if *cursor_pos < if_time.len() {
                                    if_time.remove(*cursor_pos);
                                }
                            }
                        }
                        KeyCode::Char(c) => {
                            if *active_field == 1 {
                                if_process.insert(*cursor_pos, c);
                                *cursor_pos += 1;
                            } else if *active_field == 2 {
                                if_time.insert(*cursor_pos, c);
                                *cursor_pos += 1;
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(w_name) = &selected_workflow_name {
                                if let Some(wf) = config.workflows.get_mut(w_name) {
                                    let bat = match if_battery_idx {
                                        1 => Some(true),
                                        2 => Some(false),
                                        _ => None,
                                    };
                                    let proc = if if_process.trim().is_empty() { None } else { Some(if_process.trim().to_string()) };
                                    let t = if if_time.trim().is_empty() { None } else { Some(if_time.trim().to_string()) };
                                    
                                    let has_any = bat.is_some() || proc.is_some() || t.is_some();
                                    wf.steps[*step_idx].condition = if has_any {
                                        Some(StepCondition {
                                            if_battery: bat,
                                            if_process_not_running: proc,
                                            if_time: t,
                                        })
                                    } else {
                                        None
                                    };
                                    let _ = save_config(config);
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::HealthCheckReport { reports, scroll_idx } => match key.code {
                        KeyCode::Esc | KeyCode::Enter => {
                            state = AppState::Main;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if *scroll_idx > 0 {
                                *scroll_idx -= 1;
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            if !reports.is_empty() && *scroll_idx + 1 < reports.len() {
                                *scroll_idx += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Dynamic popup sizing for smaller screen bounds
    let dynamic_x = if r.width < 80 {
        92
    } else if r.width < 120 {
        80
    } else {
        percent_x
    };

    let dynamic_y = if r.height < 24 {
        90
    } else if r.height < 40 {
        80
    } else {
        percent_y
    };

    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - dynamic_y) / 2),
            Constraint::Percentage(dynamic_y),
            Constraint::Percentage((100 - dynamic_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - dynamic_x) / 2),
            Constraint::Percentage(dynamic_x),
            Constraint::Percentage((100 - dynamic_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
