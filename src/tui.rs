use crate::config::{load_config, save_config, Config, Workflow, WorkflowStep};
use crate::desktop::{scan_desktop_entries, DesktopEntry, find_desktop_file};
use crate::launcher::{launch_workflow, execute_step};
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
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    text::{Line, Span},
    Terminal,
};
use std::io;

enum ActivePane {
    Workflows,
    Steps,
}

enum AppState {
    Main,
    NewWorkflowPrompt {
        input: String,
    },
    RenameWorkflowPrompt {
        old_name: String,
        input: String,
    },
    CloneWorkflowPrompt {
        old_name: String,
        input: String,
    },
    AddStepTypeSelector {
        selected_idx: usize,
    },
    AddAppSearch {
        search: String,
        selected_idx: usize,
        all_apps: Vec<DesktopEntry>,
    },
    AddLaunchCommandPrompt {
        input: String,
    },
    AddCustomCommandPrompt {
        input: String,
    },
    AddWaitPrompt {
        input: String,
    },
    AddNotifyPrompt {
        title_input: String,
        body_input: String,
        active_field: usize,
    },
    EditStepLaunch {
        step_idx: usize,
        desktop: String,
        workspace: String,
        silent: bool,
        monitor_cond: String,
        active_field: usize,
    },
    EditStepScript {
        step_idx: usize,
        command: String,
        dir: String,
        active_field: usize,
    },
    EditStepWait {
        step_idx: usize,
        ms: String,
    },
    EditStepNotify {
        step_idx: usize,
        title: String,
        body: String,
        active_field: usize,
    },
    HealthCheckReport {
        reports: Vec<String>,
        scroll_idx: usize,
    },
}

pub fn run_tui() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load configuration
    let mut config = load_config()?;

    // 2. Scan all system desktop entries once
    let all_desktop_apps = scan_desktop_entries();

    // 3. Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 4. Run loop
    let res = run_event_loop(&mut terminal, &mut config, all_desktop_apps);

    // 5. Restore terminal
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

    // List states for tracking UI selections
    let mut workflow_list_state = ListState::default();
    let mut step_list_state = ListState::default();

    // Select default workflow
    let mut workflows: Vec<String> = config.workflows.keys().cloned().collect();
    workflows.sort();
    if let Some(pos) = workflows.iter().position(|w| w == &config.active_workflow) {
        workflow_list_state.select(Some(pos));
    } else if !workflows.is_empty() {
        workflow_list_state.select(Some(0));
    }

    loop {
        // Prepare variables for rendering
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

        // Ensure selection bounds
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

        // Draw UI
        let selected_w_name = selected_workflow_name.clone();
        let current_steps_clone = current_steps.clone();
        let cur_workflows = current_workflows.clone();

        terminal.draw(|f| {
            let size = f.size();

            // Outer layout
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(size);

            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(main_chunks[0]);

            // Left Pane: Workflows
            let left_border_style = match active_pane {
                ActivePane::Workflows => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ActivePane::Steps => Style::default().fg(Color::DarkGray),
            };

            let workflow_items: Vec<ListItem> = cur_workflows
                .iter()
                .map(|w| {
                    let active_indicator = if w == &config.active_workflow {
                        "★"
                    } else {
                        " "
                    };
                    ListItem::new(format!("{} {}", active_indicator, w))
                })
                .collect();

            let workflow_list = List::new(workflow_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Workflows ")
                        .border_style(left_border_style),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            f.render_stateful_widget(workflow_list, body_chunks[0], &mut workflow_list_state);

            // Right Pane Layout: Split vertically into Step List and Step Details
            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(body_chunks[1]);

            // Right Pane: Workflow Detail (Steps)
            let right_border_style = match active_pane {
                ActivePane::Steps => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ActivePane::Workflows => Style::default().fg(Color::DarkGray),
            };

            let right_title = match &selected_w_name {
                Some(name) => format!(" Workflow: {} ", name),
                None => " Workflow Detail ".to_string(),
            };

            if current_steps_clone.is_empty() {
                let placeholder = Paragraph::new("\n\n   No steps configured.\n   Press 'a' to add your first step to this workflow.")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(right_title.as_str())
                            .border_style(right_border_style),
                    )
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(placeholder, right_chunks[0]);
            } else {
                let step_items: Vec<ListItem> = current_steps_clone
                    .iter()
                    .enumerate()
                    .map(|(i, step)| {
                        let step_num = i + 1;
                        match step {
                            WorkflowStep::Launch { desktop, workspace, silent, monitor_cond } => {
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
                                let (prefix, style) = if exists {
                                    ("🚀 ", Style::default())
                                } else {
                                    ("⚠️ ", Style::default().fg(Color::Yellow))
                                };
                                
                                ListItem::new(format!(
                                    "{} {}. Launch: {}{}",
                                    prefix, step_num, label, tags_str
                                )).style(style)
                            }
                            WorkflowStep::RunScript { command, dir } => {
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
                                ListItem::new(format!(
                                    "⚙️  {}. Script: {}{}",
                                    step_num, cmd_short, dir_str
                                )).style(Style::default().fg(Color::LightCyan))
                            }
                            WorkflowStep::Wait { ms } => {
                                ListItem::new(format!(
                                    "⏳ {}. Wait: {} ms",
                                    step_num, ms
                                )).style(Style::default().fg(Color::LightYellow))
                            }
                            WorkflowStep::Notify { title, body } => {
                                let body_short = if body.len() > 30 {
                                    format!("{}...", &body[..27])
                                } else {
                                    body.clone()
                                };
                                ListItem::new(format!(
                                    "🔔 {}. Notify: \"{}\" - {}",
                                    step_num, title, body_short
                                )).style(Style::default().fg(Color::LightMagenta))
                            }
                        }
                    })
                    .collect();

                let step_list = List::new(step_items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(right_title.as_str())
                            .border_style(right_border_style),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Cyan)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");

                f.render_stateful_widget(step_list, right_chunks[0], &mut step_list_state);
            }

            // Render Step Details in right_chunks[1]
            let details_block = Block::default()
                .borders(Borders::ALL)
                .title(" Step Details ")
                .border_style(Style::default().fg(Color::DarkGray));

            let mut details_lines = Vec::new();
            if let Some(idx) = step_list_state.selected() {
                if let Some(step) = current_steps_clone.get(idx) {
                    match step {
                        WorkflowStep::Launch { desktop, workspace, silent, monitor_cond } => {
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
                                Style::default().fg(Color::Red)
                            } else {
                                Style::default().fg(Color::Green)
                            };

                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Launch Application", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Target:  "),
                                Span::styled(desktop, Style::default().fg(Color::Cyan))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Config:  "),
                                Span::raw(format!(
                                    "Workspace: {} | Silent: {} | Monitor Condition: {}",
                                    workspace.as_deref().unwrap_or("Default"),
                                    if silent.unwrap_or(false) { "Yes" } else { "No" },
                                    monitor_cond.as_deref().unwrap_or("None")
                                ))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Status:  "),
                                Span::styled(status_text, status_style)
                            ]));
                        }
                        WorkflowStep::RunScript { command, dir } => {
                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Run Shell Script / Command", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Command: "),
                                Span::styled(command, Style::default().fg(Color::Cyan))
                            ]));
                            if let Some(d) = dir {
                                details_lines.push(Line::from(vec![
                                    Span::raw(" Dir:     "),
                                    Span::raw(d)
                                ]));
                            }
                        }
                        WorkflowStep::Wait { ms } => {
                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Wait / Sleep Delay", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Delay:   "),
                                Span::styled(format!("{} milliseconds", ms), Style::default().fg(Color::Cyan))
                            ]));
                        }
                        WorkflowStep::Notify { title, body } => {
                            details_lines.push(Line::from(vec![
                                Span::raw(" Type:    "),
                                Span::styled("Desktop Notification", Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Title:   "),
                                Span::styled(title, Style::default().fg(Color::Cyan))
                            ]));
                            details_lines.push(Line::from(vec![
                                Span::raw(" Body:    "),
                                Span::raw(body)
                            ]));
                        }
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

            // Bottom Help Pane
            let help_text = match active_pane {
                ActivePane::Workflows => {
                    "Enter: Launch Workflow | Space: Set Active | c: Create | r: Rename | y: Clone | d: Delete | Tab: Edit Steps | v: Health Check | q: Quit"
                }
                ActivePane::Steps => {
                    "Tab: Back | Enter: Test Run Step | a: Add Step | e: Edit Step | d: Delete Step | Shift+Up/Down: Move Step | q: Quit"
                }
            };
            let help_paragraph = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title(" Help Commands "));
            f.render_widget(help_paragraph, main_chunks[1]);

            // Draw popups based on state
            match &state {
                AppState::NewWorkflowPrompt { input } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  > {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Create New Workflow ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::RenameWorkflowPrompt { input, .. } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  > {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Rename Workflow ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::CloneWorkflowPrompt { input, .. } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  New Name: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Duplicate Workflow ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::AddStepTypeSelector { selected_idx } => {
                    let popup_area = centered_rect(50, 38, size);
                    f.render_widget(Clear, popup_area);
                    let options = vec![
                        "1. Launch Desktop Application",
                        "2. Launch Custom Window Command",
                        "3. Run Custom Command / Script in Background",
                        "4. Wait / Delay (milliseconds)",
                        "5. Send System Notification",
                    ];
                    let items: Vec<ListItem> = options.iter().enumerate().map(|(i, opt)| {
                        let style = if i == *selected_idx {
                            Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                        };
                        ListItem::new(format!("  {}", opt)).style(style)
                    }).collect();

                    let list = List::new(items)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Select Step Type to Add ")
                                .border_style(Style::default().fg(Color::Yellow))
                        );
                    f.render_widget(list, popup_area);
                }
                AppState::AddLaunchCommandPrompt { input } => {
                    let popup_area = centered_rect(60, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  Command: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Add Custom Window Command ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::AddCustomCommandPrompt { input } => {
                    let popup_area = centered_rect(60, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  Command: {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Add Custom Command / Script ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::AddWaitPrompt { input } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  Duration (ms): {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Add Wait / Delay Step ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::AddNotifyPrompt { title_input, body_input, active_field } => {
                    let popup_area = centered_rect(60, 45, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(" Add System Notification ")
                        .border_style(Style::default().fg(Color::Yellow));

                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(1)])
                        .split(inner);

                    let title_border = if *active_field == 0 {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    let body_border = if *active_field == 1 {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };

                    let p_title = Paragraph::new(format!(" {}", title_input))
                        .block(Block::default().borders(Borders::ALL).title(" Title ").border_style(title_border));
                    let p_body = Paragraph::new(format!(" {}", body_input))
                        .block(Block::default().borders(Borders::ALL).title(" Body ").border_style(body_border));

                    f.render_widget(p_title, chunks[0]);
                    f.render_widget(p_body, chunks[1]);

                    let help = Paragraph::new("\n  Tab/Arrow: Switch Fields | Enter: Add | Esc: Cancel")
                        .style(Style::default().fg(Color::DarkGray));
                    f.render_widget(help, chunks[2]);
                }
                AppState::AddAppSearch {
                    search,
                    selected_idx,
                    all_apps,
                } => {
                    let popup_area = centered_rect(70, 70, size);
                    f.render_widget(Clear, popup_area);

                    let overlay_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Min(3)])
                        .split(popup_area);

                    let search_bar = Paragraph::new(format!(" {}", search)).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Search Applications ")
                            .border_style(Style::default().fg(Color::Green)),
                    );
                    f.render_widget(search_bar, overlay_chunks[0]);

                    // Filter list
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
                            ListItem::new(format!("{}   [{}]", entry.name, entry.filename))
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
                                .bg(Color::Green)
                                .fg(Color::Black)
                                .add_modifier(Modifier::BOLD),
                        )
                        .highlight_symbol("> ");

                    f.render_stateful_widget(app_search_list, overlay_chunks[1], &mut list_state);
                }
                AppState::EditStepLaunch { desktop, workspace, silent, monitor_cond, active_field, .. } => {
                    let popup_area = centered_rect(70, 65, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(" Edit Launch Step ")
                        .border_style(Style::default().fg(Color::Yellow));

                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3), // Desktop/Command
                            Constraint::Length(3), // Workspace
                            Constraint::Length(3), // Monitor Cond
                            Constraint::Length(3), // Silent
                            Constraint::Min(1)
                        ])
                        .split(inner);

                    let styles = [
                        if *active_field == 0 { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) },
                        if *active_field == 1 { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) },
                        if *active_field == 2 { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) },
                        if *active_field == 3 { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) },
                    ];

                    let p_desktop = Paragraph::new(format!(" {}", desktop))
                        .block(Block::default().borders(Borders::ALL).title(" Desktop File / Command ").border_style(styles[0]));
                    let p_ws = Paragraph::new(format!(" {}", workspace))
                        .block(Block::default().borders(Borders::ALL).title(" Target Workspace (optional) ").border_style(styles[1]));
                    let p_mc = Paragraph::new(format!(" {}", monitor_cond))
                        .block(Block::default().borders(Borders::ALL).title(" Monitor Condition (e.g. HDMI-A-1?3:1) ").border_style(styles[2]));

                    let silent_val = if *silent { "[X] Enabled" } else { "[ ] Disabled" };
                    let p_silent = Paragraph::new(format!("  {} (Press Space to Toggle)", silent_val))
                        .block(Block::default().borders(Borders::ALL).title(" Silent Run ").border_style(styles[3]));

                    f.render_widget(p_desktop, chunks[0]);
                    f.render_widget(p_ws, chunks[1]);
                    f.render_widget(p_mc, chunks[2]);
                    f.render_widget(p_silent, chunks[3]);

                    let help = Paragraph::new("\n  Tab/Arrow: Switch Fields | Enter: Save | Esc: Cancel")
                        .style(Style::default().fg(Color::DarkGray));
                    f.render_widget(help, chunks[4]);
                }
                AppState::EditStepScript { command, dir, active_field, .. } => {
                    let popup_area = centered_rect(70, 45, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(" Edit Script / Command Step ")
                        .border_style(Style::default().fg(Color::Yellow));

                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3), // Command
                            Constraint::Length(3), // Working Dir
                            Constraint::Min(1)
                        ])
                        .split(inner);

                    let styles = [
                        if *active_field == 0 { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) },
                        if *active_field == 1 { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) },
                    ];

                    let p_cmd = Paragraph::new(format!(" {}", command))
                        .block(Block::default().borders(Borders::ALL).title(" Command / Script ").border_style(styles[0]));
                    let p_dir = Paragraph::new(format!(" {}", dir))
                        .block(Block::default().borders(Borders::ALL).title(" Working Directory (optional) ").border_style(styles[1]));

                    f.render_widget(p_cmd, chunks[0]);
                    f.render_widget(p_dir, chunks[1]);

                    let help = Paragraph::new("\n  Tab/Arrow: Switch Fields | Enter: Save | Esc: Cancel")
                        .style(Style::default().fg(Color::DarkGray));
                    f.render_widget(help, chunks[2]);
                }
                AppState::EditStepWait { ms, .. } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  Duration (ms): {}", ms))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Edit Wait Step ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::EditStepNotify { title, body, active_field, .. } => {
                    let popup_area = centered_rect(60, 45, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(" Edit Notification Step ")
                        .border_style(Style::default().fg(Color::Yellow));

                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Min(1)])
                        .split(inner);

                    let styles = [
                        if *active_field == 0 { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) },
                        if *active_field == 1 { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::DarkGray) },
                    ];

                    let p_title = Paragraph::new(format!(" {}", title))
                        .block(Block::default().borders(Borders::ALL).title(" Title ").border_style(styles[0]));
                    let p_body = Paragraph::new(format!(" {}", body))
                        .block(Block::default().borders(Borders::ALL).title(" Body ").border_style(styles[1]));

                    f.render_widget(p_title, chunks[0]);
                    f.render_widget(p_body, chunks[1]);

                    let help = Paragraph::new("\n  Tab/Arrow: Switch Fields | Enter: Save | Esc: Cancel")
                        .style(Style::default().fg(Color::DarkGray));
                    f.render_widget(help, chunks[2]);
                }
                AppState::HealthCheckReport { reports, scroll_idx } => {
                    let popup_area = centered_rect(75, 75, size);
                    f.render_widget(Clear, popup_area);

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(" Workflow Health Check Results ")
                        .border_style(Style::default().fg(Color::Cyan));

                    let inner = block.inner(popup_area);
                    f.render_widget(block, popup_area);

                    let items: Vec<ListItem> = if reports.is_empty() {
                        vec![ListItem::new("  ✔ All steps are healthy! No issues found.")]
                    } else {
                        reports.iter().map(|r| {
                            ListItem::new(format!("  {}", r))
                        }).collect()
                    };

                    let mut list_state = ListState::default();
                    if !reports.is_empty() {
                        list_state.select(Some(*scroll_idx));
                    }

                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(3), Constraint::Length(2)])
                        .split(inner);

                    let list = List::new(items)
                        .highlight_style(Style::default().bg(Color::DarkGray).fg(Color::White))
                        .highlight_symbol("> ");

                    f.render_stateful_widget(list, chunks[0], &mut list_state);

                    let footer = Paragraph::new("Press Enter or Esc to dismiss | Up/Down to scroll")
                        .style(Style::default().fg(Color::DarkGray));
                    f.render_widget(footer, chunks[1]);
                }
                AppState::Main => {}
            }
        })?;

        // Read event
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Global exit safety
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(());
                }

                match &mut state {
                    AppState::Main => {
                        match active_pane {
                            ActivePane::Workflows => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
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
                                KeyCode::Char('c') => {
                                    state = AppState::NewWorkflowPrompt {
                                        input: String::new(),
                                    };
                                }
                                KeyCode::Char('r') => {
                                    if let Some(w_name) = &selected_workflow_name {
                                        state = AppState::RenameWorkflowPrompt {
                                            old_name: w_name.clone(),
                                            input: w_name.clone(),
                                        };
                                    }
                                }
                                KeyCode::Char('y') => {
                                    if let Some(w_name) = &selected_workflow_name {
                                        state = AppState::CloneWorkflowPrompt {
                                            old_name: w_name.clone(),
                                            input: format!("{}-copy", w_name),
                                        };
                                    }
                                }
                                KeyCode::Char(' ') => {
                                    if let Some(w_name) = &selected_workflow_name {
                                        config.active_workflow = w_name.clone();
                                        let _ = save_config(config);
                                    }
                                }
                                KeyCode::Char('d') | KeyCode::Delete => {
                                    if let Some(w_name) = &selected_workflow_name {
                                        config.workflows.remove(w_name);
                                        // If deleted active workflow, reset active_workflow
                                        if &config.active_workflow == w_name {
                                            config.active_workflow = config
                                                .workflows
                                                .keys()
                                                .next()
                                                .cloned()
                                                .unwrap_or_else(|| "default".to_string());
                                        }
                                        let _ = save_config(config);
                                    }
                                }
                                KeyCode::Char('v') => {
                                    if let Some(w_name) = &selected_workflow_name {
                                        if let Some(wf) = config.workflows.get(w_name) {
                                            let reports = validate_workflow_steps(&wf.steps);
                                            state = AppState::HealthCheckReport {
                                                reports,
                                                scroll_idx: 0,
                                            };
                                        }
                                    }
                                }
                                KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                                    active_pane = ActivePane::Steps;
                                    if !current_steps.is_empty() {
                                        step_list_state.select(Some(0));
                                    } else {
                                        step_list_state.select(None);
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(w_name) = &selected_workflow_name {
                                        config.active_workflow = w_name.clone();
                                        let _ = save_config(config);
                                        // Launch and exit TUI!
                                        let _ = launch_workflow(&current_steps);
                                        return Ok(());
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
                                        // Reorder up
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
                                        // Reorder down
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
                                            match step {
                                                WorkflowStep::Launch { desktop, workspace, silent, monitor_cond } => {
                                                    state = AppState::EditStepLaunch {
                                                        step_idx: idx,
                                                        desktop: desktop.clone(),
                                                        workspace: workspace.clone().unwrap_or_default(),
                                                        silent: silent.unwrap_or(false),
                                                        monitor_cond: monitor_cond.clone().unwrap_or_default(),
                                                        active_field: 0,
                                                    };
                                                }
                                                WorkflowStep::RunScript { command, dir } => {
                                                    state = AppState::EditStepScript {
                                                        step_idx: idx,
                                                        command: command.clone(),
                                                        dir: dir.clone().unwrap_or_default(),
                                                        active_field: 0,
                                                    };
                                                }
                                                WorkflowStep::Wait { ms } => {
                                                    state = AppState::EditStepWait {
                                                        step_idx: idx,
                                                        ms: ms.to_string(),
                                                    };
                                                }
                                                WorkflowStep::Notify { title, body } => {
                                                    state = AppState::EditStepNotify {
                                                        step_idx: idx,
                                                        title: title.clone(),
                                                        body: body.clone(),
                                                        active_field: 0,
                                                    };
                                                }
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('d') | KeyCode::Delete => {
                                    if let (Some(w_name), Some(idx)) = (&selected_workflow_name, step_list_state.selected()) {
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
                        }
                    }
                    AppState::NewWorkflowPrompt { input } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim();
                            if !trimmed.is_empty() {
                                config.workflows.insert(trimmed.to_string(), Workflow {
                                    name: trimmed.to_string(),
                                    description: None,
                                    project_path: None,
                                    steps: Vec::new(),
                                });
                                config.active_workflow = trimmed.to_string();
                                let _ = save_config(config);

                                let mut w: Vec<String> = config.workflows.keys().cloned().collect();
                                w.sort();
                                if let Some(pos) = w.iter().position(|name| name == trimmed) {
                                    workflow_list_state.select(Some(pos));
                                }
                            }
                            state = AppState::Main;
                        }
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Char(c) => {
                            input.push(c);
                        }
                        _ => {}
                    },
                    AppState::RenameWorkflowPrompt { old_name, input } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim();
                            if !trimmed.is_empty() && trimmed != old_name {
                                if let Some(mut wf) = config.workflows.remove(old_name) {
                                    wf.name = trimmed.to_string();
                                    config.workflows.insert(trimmed.to_string(), wf);
                                    if config.active_workflow == *old_name {
                                        config.active_workflow = trimmed.to_string();
                                    }
                                    let _ = save_config(config);

                                    let mut w: Vec<String> = config.workflows.keys().cloned().collect();
                                    w.sort();
                                    if let Some(pos) = w.iter().position(|name| name == trimmed) {
                                        workflow_list_state.select(Some(pos));
                                    }
                                }
                            }
                            state = AppState::Main;
                        }
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Char(c) => {
                            input.push(c);
                        }
                        _ => {}
                    },
                    AppState::CloneWorkflowPrompt { old_name, input } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim();
                            if !trimmed.is_empty() && trimmed != old_name {
                                if let Some(wf) = config.workflows.get(old_name) {
                                    let mut wf_clone = wf.clone();
                                    wf_clone.name = trimmed.to_string();
                                    config.workflows.insert(trimmed.to_string(), wf_clone);
                                    let _ = save_config(config);

                                    let mut w: Vec<String> = config.workflows.keys().cloned().collect();
                                    w.sort();
                                    if let Some(pos) = w.iter().position(|name| name == trimmed) {
                                        workflow_list_state.select(Some(pos));
                                    }
                                }
                            }
                            state = AppState::Main;
                        }
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Char(c) => {
                            input.push(c);
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
                            if *selected_idx < 4 {
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
                                    };
                                }
                                1 => {
                                    state = AppState::AddLaunchCommandPrompt {
                                        input: String::new(),
                                    };
                                }
                                2 => {
                                    state = AppState::AddCustomCommandPrompt {
                                        input: String::new(),
                                    };
                                }
                                3 => {
                                    state = AppState::AddWaitPrompt {
                                        input: String::new(),
                                    };
                                }
                                4 => {
                                    state = AppState::AddNotifyPrompt {
                                        title_input: String::new(),
                                        body_input: String::new(),
                                        active_field: 0,
                                    };
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    },
                    AppState::AddLaunchCommandPrompt { input } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep::Launch {
                                            desktop: trimmed,
                                            workspace: None,
                                            silent: Some(false),
                                            monitor_cond: None,
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
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Char(c) => {
                            input.push(c);
                        }
                        _ => {}
                    },
                    AppState::AddCustomCommandPrompt { input } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim().to_string();
                            if !trimmed.is_empty() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep::RunScript {
                                            command: trimmed,
                                            dir: None,
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
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Char(c) => {
                            input.push(c);
                        }
                        _ => {}
                    },
                    AppState::AddWaitPrompt { input } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim();
                            if let Ok(ms) = trimmed.parse::<u64>() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep::Wait { ms });
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
                        KeyCode::Backspace => {
                            input.pop();
                        }
                        KeyCode::Char(c) => {
                            if c.is_ascii_digit() {
                                input.push(c);
                            }
                        }
                        _ => {}
                    },
                    AppState::AddNotifyPrompt { title_input, body_input, active_field } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down | KeyCode::Up => {
                            *active_field = if *active_field == 0 { 1 } else { 0 };
                        }
                        KeyCode::Enter => {
                            let title = title_input.trim().to_string();
                            let body = body_input.trim().to_string();
                            if !title.is_empty() || !body.is_empty() {
                                if let Some(w_name) = &selected_workflow_name {
                                    let new_len = if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps.push(WorkflowStep::Notify { title, body });
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
                        KeyCode::Backspace => {
                            if *active_field == 0 {
                                title_input.pop();
                            } else {
                                body_input.pop();
                            }
                        }
                        KeyCode::Char(c) => {
                            if *active_field == 0 {
                                title_input.push(c);
                            } else {
                                body_input.push(c);
                            }
                        }
                        _ => {}
                    },
                    AppState::AddAppSearch {
                        search,
                        selected_idx,
                        all_apps,
                    } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Backspace => {
                            search.pop();
                            *selected_idx = 0;
                        }
                        KeyCode::Char(c) => {
                            search.push(c);
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
                                        wf.steps.push(WorkflowStep::Launch {
                                            desktop: selected_app.filename.clone(),
                                            workspace: None,
                                            silent: Some(false),
                                            monitor_cond: None,
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
                    AppState::EditStepLaunch { step_idx, desktop, workspace, silent, monitor_cond, active_field } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down => {
                            *active_field = (*active_field + 1) % 4;
                        }
                        KeyCode::Up => {
                            *active_field = (*active_field + 3) % 4;
                        }
                        KeyCode::Char(' ') if *active_field == 3 => {
                            *silent = !*silent;
                        }
                        KeyCode::Backspace => {
                            match active_field {
                                0 => { desktop.pop(); }
                                1 => { workspace.pop(); }
                                2 => { monitor_cond.pop(); }
                                _ => {}
                            }
                        }
                        KeyCode::Char(c) => {
                            match active_field {
                                0 => { desktop.push(c); }
                                1 => { workspace.push(c); }
                                2 => { monitor_cond.push(c); }
                                _ => {}
                            }
                        }
                        KeyCode::Enter => {
                            if let Some(w_name) = &selected_workflow_name {
                                if let Some(wf) = config.workflows.get_mut(w_name) {
                                    let ws = if workspace.trim().is_empty() { None } else { Some(workspace.trim().to_string()) };
                                    let mc = if monitor_cond.trim().is_empty() { None } else { Some(monitor_cond.trim().to_string()) };
                                    wf.steps[*step_idx] = WorkflowStep::Launch {
                                        desktop: desktop.trim().to_string(),
                                        workspace: ws,
                                        silent: Some(*silent),
                                        monitor_cond: mc,
                                    };
                                    let _ = save_config(config);
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepScript { step_idx, command, dir, active_field } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down | KeyCode::Up => {
                            *active_field = if *active_field == 0 { 1 } else { 0 };
                        }
                        KeyCode::Backspace => {
                            if *active_field == 0 { command.pop(); } else { dir.pop(); }
                        }
                        KeyCode::Char(c) => {
                            if *active_field == 0 { command.push(c); } else { dir.push(c); }
                        }
                        KeyCode::Enter => {
                            if let Some(w_name) = &selected_workflow_name {
                                if let Some(wf) = config.workflows.get_mut(w_name) {
                                    let d = if dir.trim().is_empty() { None } else { Some(dir.trim().to_string()) };
                                    wf.steps[*step_idx] = WorkflowStep::RunScript {
                                        command: command.trim().to_string(),
                                        dir: d,
                                    };
                                    let _ = save_config(config);
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepWait { step_idx, ms } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Backspace => {
                            ms.pop();
                        }
                        KeyCode::Char(c) => {
                            if c.is_ascii_digit() {
                                ms.push(c);
                            }
                        }
                        KeyCode::Enter => {
                            if let Ok(duration) = ms.trim().parse::<u64>() {
                                if let Some(w_name) = &selected_workflow_name {
                                    if let Some(wf) = config.workflows.get_mut(w_name) {
                                        wf.steps[*step_idx] = WorkflowStep::Wait { ms: duration };
                                        let _ = save_config(config);
                                    }
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
                    AppState::EditStepNotify { step_idx, title, body, active_field } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Tab | KeyCode::Down | KeyCode::Up => {
                            *active_field = if *active_field == 0 { 1 } else { 0 };
                        }
                        KeyCode::Backspace => {
                            if *active_field == 0 { title.pop(); } else { body.pop(); }
                        }
                        KeyCode::Char(c) => {
                            if *active_field == 0 { title.push(c); } else { body.push(c); }
                        }
                        KeyCode::Enter => {
                            if let Some(w_name) = &selected_workflow_name {
                                if let Some(wf) = config.workflows.get_mut(w_name) {
                                    wf.steps[*step_idx] = WorkflowStep::Notify {
                                        title: title.trim().to_string(),
                                        body: body.trim().to_string(),
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
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
