use crate::config::{load_config, save_config, Config, ProfileApp};
use crate::desktop::{scan_desktop_entries, DesktopEntry};
use crate::launcher::launch_profile;

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
    Profiles,
    Apps,
}

enum AppState {
    Main,
    NewProfilePrompt {
        input: String,
    },
    RenameProfilePrompt {
        old_name: String,
        input: String,
    },
    AddAppSearch {
        search: String,
        selected_idx: usize,
        all_apps: Vec<DesktopEntry>,
    },
    EditWorkspacePrompt {
        input: String,
        app_idx: usize,
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
    let mut active_pane = ActivePane::Profiles;
    let mut state = AppState::Main;

    // List states for tracking UI selections
    let mut profile_list_state = ListState::default();
    let mut app_list_state = ListState::default();

    // Select default profile
    let mut profiles: Vec<String> = config.profiles.keys().cloned().collect();
    profiles.sort();
    if let Some(pos) = profiles.iter().position(|p| p == &config.active_profile) {
        profile_list_state.select(Some(pos));
    } else if !profiles.is_empty() {
        profile_list_state.select(Some(0));
    }

    loop {
        // Prepare variables for rendering
        let current_profiles: Vec<String> = {
            let mut p: Vec<String> = config.profiles.keys().cloned().collect();
            p.sort();
            p
        };

        let selected_profile_name = profile_list_state
            .selected()
            .and_then(|idx| current_profiles.get(idx))
            .cloned();

        let current_apps = selected_profile_name
            .as_ref()
            .and_then(|name| config.profiles.get(name))
            .cloned()
            .unwrap_or_default();

        // Ensure selection bounds
        if profile_list_state.selected().is_some() && current_profiles.is_empty() {
            profile_list_state.select(None);
        } else if profile_list_state.selected().is_none() && !current_profiles.is_empty() {
            profile_list_state.select(Some(0));
        }

        if app_list_state.selected().is_some() && current_apps.is_empty() {
            app_list_state.select(None);
        } else if app_list_state.selected().is_none() && !current_apps.is_empty() {
            app_list_state.select(Some(0));
        }

        // Draw UI
        let selected_p_name = selected_profile_name.clone();
        let current_apps_clone = current_apps.clone();
        let cur_profiles = current_profiles.clone();

        terminal.draw(|f| {
            let size = f.size();

            // Background / outer layout
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(size);

            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(main_chunks[0]);

            // Left Pane: Profiles
            let left_border_style = match active_pane {
                ActivePane::Profiles => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ActivePane::Apps => Style::default().fg(Color::DarkGray),
            };

            let profile_items: Vec<ListItem> = cur_profiles
                .iter()
                .map(|p| {
                    let active_indicator = if p == &config.active_profile {
                        " (active)"
                    } else {
                        ""
                    };
                    ListItem::new(format!("{}{}", p, active_indicator))
                })
                .collect();

            let profile_list = List::new(profile_items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Profiles ")
                        .border_style(left_border_style),
                )
                .highlight_style(
                    Style::default()
                        .bg(Color::Cyan)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");

            f.render_stateful_widget(profile_list, body_chunks[0], &mut profile_list_state);

            // Right Pane Layout: Split vertically into App List and App Details
            let right_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(body_chunks[1]);

            // Right Pane: Profile Detail (Apps)
            let right_border_style = match active_pane {
                ActivePane::Apps => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ActivePane::Profiles => Style::default().fg(Color::DarkGray),
            };

            let right_title = match &selected_p_name {
                Some(name) => format!(" Profile: {} ", name),
                None => " Profile Detail ".to_string(),
            };

            if current_apps_clone.is_empty() {
                let placeholder = Paragraph::new("\n\n   No applications configured.\n   Press 'a' to add your first application.")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(right_title.as_str())
                            .border_style(right_border_style),
                    )
                    .style(Style::default().fg(Color::DarkGray));
                f.render_widget(placeholder, right_chunks[0]);
            } else {
                let app_items: Vec<ListItem> = current_apps_clone
                    .iter()
                    .map(|app| {
                        let ws_str = app
                            .workspace
                            .as_ref()
                            .map(|w| format!("Workspace: {}", w))
                            .unwrap_or_else(|| "Workspace: Default".to_string());
                        let silent_str = if app.silent.unwrap_or(false) {
                            " [Silent]"
                        } else {
                            ""
                        };

                        let exists = all_desktop_apps.iter().any(|e| e.filename == app.desktop);
                        let (prefix, style) = if exists {
                            ("  ", Style::default())
                        } else {
                            ("⚠️ ", Style::default().fg(Color::Yellow))
                        };

                        ListItem::new(format!(
                            "{}{}   ({}{})",
                            prefix, app.desktop, ws_str, silent_str
                        )).style(style)
                    })
                    .collect();

                let app_list = List::new(app_items)
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

                f.render_stateful_widget(app_list, right_chunks[0], &mut app_list_state);
            }

            // Render App Details in right_chunks[1]
            let details_block = Block::default()
                .borders(Borders::ALL)
                .title(" Application Details ")
                .border_style(Style::default().fg(Color::DarkGray));

            let mut details_lines = Vec::new();
            if let (Some(apps), Some(idx)) = (selected_p_name.as_ref().and_then(|n| config.profiles.get(n)), app_list_state.selected()) {
                if let Some(app) = apps.get(idx) {
                    if let Some(entry) = all_desktop_apps.iter().find(|e| e.filename == app.desktop) {
                        let ws_str = app.workspace.as_ref().map(|w| w.as_str()).unwrap_or("Default");
                        let silent_str = if app.silent.unwrap_or(false) { "Yes" } else { "No" };
                        let exec_str = entry.exec.join(" ");
                        let path_str = entry.path.as_deref().unwrap_or("None");

                        let desktop_path = entry.file_path.to_string_lossy().into_owned();
                        details_lines.push(Line::from(vec![Span::raw(" Name:         "), Span::styled(&entry.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD))]));
                        details_lines.push(Line::from(vec![Span::raw(" Desktop File: "), Span::styled(desktop_path, Style::default().fg(Color::Gray))]));
                        details_lines.push(Line::from(vec![Span::raw(" Command:      "), Span::styled(exec_str, Style::default().fg(Color::Cyan))]));
                        details_lines.push(Line::from(vec![Span::raw(" Working Dir:  "), Span::raw(path_str)]));
                        details_lines.push(Line::from(vec![Span::raw(" Workspace:    "), Span::styled(ws_str, Style::default().fg(Color::Magenta))]));
                        details_lines.push(Line::from(vec![Span::raw(" Silent Run:   "), Span::raw(silent_str)]));
                    } else {
                        details_lines.push(Line::from(vec![Span::styled(format!(" ⚠️ Unknown Application ({})", app.desktop), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]));
                        details_lines.push(Line::from(vec![Span::raw(" Warning:      Desktop file not found on the system.")]));
                    }
                } else {
                    details_lines.push(Line::from(" No application selected."));
                }
            } else {
                details_lines.push(Line::from(" Select an application to view details."));
            };

            let details_paragraph = Paragraph::new(details_lines)
                .block(details_block);
            f.render_widget(details_paragraph, right_chunks[1]);

            // Bottom Help Pane
            let help_text = match active_pane {
                ActivePane::Profiles => {
                    "Enter: Launch | c: Create | r: Rename | d: Delete | Tab: Edit Apps | q: Quit"
                }
                ActivePane::Apps => {
                    "Tab: Back | a: Add App | d: Delete App | w: Workspace | s: Toggle Silent | Shift+Up/Down: Move"
                }
            };
            let help_paragraph = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title(" Help Commands "));
            f.render_widget(help_paragraph, main_chunks[1]);

            // Draw popups based on state
            match &state {
                AppState::NewProfilePrompt { input } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  > {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Create New Profile ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::RenameProfilePrompt { input, .. } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  > {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Rename Profile ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
                }
                AppState::EditWorkspacePrompt { input, .. } => {
                    let popup_area = centered_rect(50, 20, size);
                    f.render_widget(Clear, popup_area);
                    let input_block = Paragraph::new(format!("\n  Workspace (e.g. 1, 2, silent): {}", input))
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(" Set Workspace (Leave empty for default) ")
                                .border_style(Style::default().fg(Color::Yellow)),
                        );
                    f.render_widget(input_block, popup_area);
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
                            ActivePane::Profiles => match key.code {
                                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                KeyCode::Up | KeyCode::Char('k') => {
                                    if let Some(selected) = profile_list_state.selected() {
                                        if selected > 0 {
                                            profile_list_state.select(Some(selected - 1));
                                        }
                                    }
                                }
                                KeyCode::Down | KeyCode::Char('j') => {
                                    if let Some(selected) = profile_list_state.selected() {
                                        if selected + 1 < current_profiles.len() {
                                            profile_list_state.select(Some(selected + 1));
                                        }
                                    }
                                }
                                KeyCode::Char('c') => {
                                    state = AppState::NewProfilePrompt {
                                        input: String::new(),
                                    };
                                }
                                KeyCode::Char('r') => {
                                    if let Some(p_name) = &selected_profile_name {
                                        state = AppState::RenameProfilePrompt {
                                            old_name: p_name.clone(),
                                            input: p_name.clone(),
                                        };
                                    }
                                }
                                KeyCode::Char('d') | KeyCode::Delete => {
                                    if let Some(p_name) = &selected_profile_name {
                                        config.profiles.remove(p_name);
                                        // If deleted active profile, reset active_profile
                                        if &config.active_profile == p_name {
                                            config.active_profile = config
                                                .profiles
                                                .keys()
                                                .next()
                                                .cloned()
                                                .unwrap_or_else(|| "default".to_string());
                                        }
                                        let _ = save_config(config);
                                    }
                                }
                                KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                                    active_pane = ActivePane::Apps;
                                    if !current_apps.is_empty() {
                                        app_list_state.select(Some(0));
                                    } else {
                                        app_list_state.select(None);
                                    }
                                }
                                KeyCode::Enter => {
                                    if let Some(p_name) = &selected_profile_name {
                                        config.active_profile = p_name.clone();
                                        let _ = save_config(config);
                                        // Launch and exit TUI!
                                        let _ = launch_profile(&current_apps);
                                        return Ok(());
                                    }
                                }
                                _ => {}
                            },
                            ActivePane::Apps => match key.code {
                                KeyCode::Tab | KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => {
                                    active_pane = ActivePane::Profiles;
                                }
                                KeyCode::Up => {
                                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                                        // Reorder up
                                        if let (Some(p_name), Some(idx)) = (&selected_profile_name, app_list_state.selected()) {
                                            if idx > 0 {
                                                if let Some(apps) = config.profiles.get_mut(p_name) {
                                                    apps.swap(idx, idx - 1);
                                                    app_list_state.select(Some(idx - 1));
                                                    let _ = save_config(config);
                                                }
                                            }
                                        }
                                    } else if let Some(selected) = app_list_state.selected() {
                                        if selected > 0 {
                                            app_list_state.select(Some(selected - 1));
                                        }
                                    }
                                }
                                KeyCode::Down => {
                                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                                        // Reorder down
                                        if let (Some(p_name), Some(idx)) = (&selected_profile_name, app_list_state.selected()) {
                                            if idx + 1 < current_apps.len() {
                                                if let Some(apps) = config.profiles.get_mut(p_name) {
                                                    apps.swap(idx, idx + 1);
                                                    app_list_state.select(Some(idx + 1));
                                                    let _ = save_config(config);
                                                }
                                            }
                                        }
                                    } else if let Some(selected) = app_list_state.selected() {
                                        if selected + 1 < current_apps.len() {
                                            app_list_state.select(Some(selected + 1));
                                        }
                                    }
                                }
                                KeyCode::Char('a') => {
                                    state = AppState::AddAppSearch {
                                        search: String::new(),
                                        selected_idx: 0,
                                        all_apps: all_desktop_apps.clone(),
                                    };
                                }
                                KeyCode::Char('d') | KeyCode::Delete => {
                                    if let (Some(p_name), Some(idx)) = (&selected_profile_name, app_list_state.selected()) {
                                        let apps_empty = if let Some(apps) = config.profiles.get_mut(p_name) {
                                            apps.remove(idx);
                                            let empty = apps.is_empty();
                                            if !empty {
                                                app_list_state.select(Some(idx.min(apps.len() - 1)));
                                            }
                                            empty
                                        } else {
                                            false
                                        };

                                        if apps_empty {
                                            active_pane = ActivePane::Profiles;
                                        }
                                        let _ = save_config(config);
                                    }
                                }
                                KeyCode::Char('w') => {
                                    if let Some(idx) = app_list_state.selected() {
                                        let current_ws = current_apps[idx].workspace.clone().unwrap_or_default();
                                        state = AppState::EditWorkspacePrompt {
                                            input: current_ws,
                                            app_idx: idx,
                                        };
                                    }
                                }

                                KeyCode::Char('s') => {
                                    if let (Some(p_name), Some(idx)) = (&selected_profile_name, app_list_state.selected()) {
                                        if let Some(apps) = config.profiles.get_mut(p_name) {
                                            let cur_silent = apps[idx].silent.unwrap_or(false);
                                            apps[idx].silent = Some(!cur_silent);
                                            let _ = save_config(config);
                                        }
                                    }
                                }
                                KeyCode::Char('q') => return Ok(()),
                                _ => {}
                            },
                        }
                    }
                    AppState::NewProfilePrompt { input } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim();
                            if !trimmed.is_empty() {
                                config.profiles.entry(trimmed.to_string()).or_default();
                                config.active_profile = trimmed.to_string();
                                let _ = save_config(config);

                                // Reset list profiles list selection to the new profile
                                let mut p: Vec<String> = config.profiles.keys().cloned().collect();
                                p.sort();
                                if let Some(pos) = p.iter().position(|name| name == trimmed) {
                                    profile_list_state.select(Some(pos));
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
                    AppState::RenameProfilePrompt { old_name, input } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            let trimmed = input.trim();
                            if !trimmed.is_empty() && trimmed != old_name {
                                if let Some(apps) = config.profiles.remove(old_name) {
                                    config.profiles.insert(trimmed.to_string(), apps);
                                    if config.active_profile == *old_name {
                                        config.active_profile = trimmed.to_string();
                                    }
                                    let _ = save_config(config);

                                    // Reset selection to renamed profile
                                    let mut p: Vec<String> = config.profiles.keys().cloned().collect();
                                    p.sort();
                                    if let Some(pos) = p.iter().position(|name| name == trimmed) {
                                        profile_list_state.select(Some(pos));
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
                    AppState::EditWorkspacePrompt { input, app_idx } => match key.code {
                        KeyCode::Esc => {
                            state = AppState::Main;
                        }
                        KeyCode::Enter => {
                            if let Some(p_name) = &selected_profile_name {
                                if let Some(apps) = config.profiles.get_mut(p_name) {
                                    let trimmed = input.trim();
                                    apps[*app_idx].workspace = if trimmed.is_empty() {
                                        None
                                    } else {
                                        Some(trimmed.to_string())
                                    };
                                    let _ = save_config(config);
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
                            // Find the selected app in the filtered list
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

                                if let Some(p_name) = &selected_profile_name {
                                    let new_len = if let Some(apps) = config.profiles.get_mut(p_name) {
                                        apps.push(ProfileApp {
                                            desktop: selected_app.filename.clone(),
                                            workspace: None,
                                            silent: Some(false),
                                        });
                                        apps.len()
                                    } else {
                                        config.profiles.insert(p_name.clone(), vec![ProfileApp {
                                            desktop: selected_app.filename.clone(),
                                            workspace: None,
                                            silent: Some(false),
                                        }]);
                                        1
                                    };

                                    let _ = save_config(config);

                                    active_pane = ActivePane::Apps;
                                    app_list_state.select(Some(new_len - 1));
                                }
                            }
                            state = AppState::Main;
                        }
                        _ => {}
                    },
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
