use crate::config::{Config, ProfileApp, load_config, save_config};
use crate::desktop::{scan_desktop_entries, DesktopEntry};
use crate::launcher::{launch_profile, launch_app_now, is_app_running};
use eframe::egui;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct HyprlaunchApp {
    config: Config,
    all_desktop_apps: Vec<DesktopEntry>,
    selected_profile: Option<String>,
    
    // Unsaved changes tracking
    is_dirty: bool,
    
    // Add App state
    show_add_app_search: bool,
    search_query: String,
    
    // New Profile state
    show_new_profile_dialog: bool,
    new_profile_name: String,
    
    // Rename Profile state
    is_renaming_profile: bool,
    rename_profile_name: String,
    
    // Running status cache
    running_status: HashMap<String, bool>,
    last_status_check: Instant,
}

impl HyprlaunchApp {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let config = load_config()?;
        let all_desktop_apps = scan_desktop_entries();
        let selected_profile = if config.profiles.contains_key(&config.active_profile) {
            Some(config.active_profile.clone())
        } else {
            config.profiles.keys().next().cloned()
        };

        let mut app = Self {
            config,
            all_desktop_apps,
            selected_profile,
            is_dirty: false,
            show_add_app_search: false,
            search_query: String::new(),
            show_new_profile_dialog: false,
            new_profile_name: String::new(),
            is_renaming_profile: false,
            rename_profile_name: String::new(),
            running_status: HashMap::new(),
            last_status_check: Instant::now() - Duration::from_secs(10), // force initial check
        };
        app.update_running_statuses();
        Ok(app)
    }

    fn update_running_statuses(&mut self) {
        let mut status_map = HashMap::new();
        for entry in &self.all_desktop_apps {
            let running = is_app_running(entry);
            status_map.insert(entry.filename.clone(), running);
        }
        self.running_status = status_map;
    }
}

impl eframe::App for HyprlaunchApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx();

        // Refresh running status once every second
        if self.last_status_check.elapsed() >= Duration::from_secs(1) {
            self.update_running_statuses();
            self.last_status_check = Instant::now();
        }

        // Custom styling for modern visual aesthetics
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(18, 22, 33);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(28, 33, 48);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(38, 45, 66);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(48, 57, 84);
        ctx.set_visuals(visuals);

        // Request repaint every 1 second to update the process indicators
        ctx.request_repaint_after(Duration::from_secs(1));

        // Top Control Panel
        egui::Panel::top("top_bar").show_inside(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.heading(egui::RichText::new("hyprlaunch").strong().color(egui::Color32::from_rgb(0, 242, 254)));
                        ui.label(egui::RichText::new("v0.2.1").weak());
                    });
                    ui.label(egui::RichText::new("Startup & Session Manager for Hyprland").small().weak());
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Launch Active Profile Button
                    let active_profile = self.config.active_profile.clone();
                    if !active_profile.is_empty() {
                        let launch_btn = egui::Button::new(
                            egui::RichText::new(format!("🚀 Launch Active: {}", active_profile))
                                .strong()
                                .color(egui::Color32::BLACK)
                        ).fill(egui::Color32::from_rgb(0, 242, 254));

                        if ui.add(launch_btn).clicked() {
                            if let Some(apps) = self.config.profiles.get(&active_profile) {
                                let apps_clone = apps.clone();
                                std::thread::spawn(move || {
                                    let _ = launch_profile(&apps_clone);
                                });
                            }
                        }
                    }

                    // Save Config Button
                    let save_text = if self.is_dirty { "💾 Save Config (Unsaved Changes)" } else { "💾 Save Config" };
                    let save_btn = if self.is_dirty {
                        egui::Button::new(save_text).fill(egui::Color32::from_rgb(212, 163, 89))
                    } else {
                        egui::Button::new(save_text)
                    };
                    if ui.add(save_btn).clicked() {
                        if save_config(&self.config).is_ok() {
                            self.is_dirty = false;
                        }
                    }
                });
            });
            ui.add_space(8.0);
        });

        // Sidebar (Profiles List)
        egui::Panel::left("profiles_sidebar")
            .resizable(true)
            .default_size(200.0)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("PROFILES").strong().weak());
                });
                ui.add_space(4.0);

                let mut to_delete_profile = None;

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut profiles: Vec<String> = self.config.profiles.keys().cloned().collect();
                    profiles.sort();

                    for p_name in profiles {
                        let is_selected = Some(p_name.clone()) == self.selected_profile;
                        let is_active = p_name == self.config.active_profile;

                        ui.horizontal(|ui| {
                            let label_text = if is_active {
                                format!("★ {}", p_name)
                            } else {
                                p_name.clone()
                            };

                            let resp = ui.selectable_label(is_selected, label_text);
                            if resp.clicked() {
                                self.selected_profile = Some(p_name.clone());
                                self.is_renaming_profile = false;
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // Deletion button
                                if ui.button("🗑").on_hover_text("Delete Profile").clicked() {
                                    to_delete_profile = Some(p_name.clone());
                                }
                                // Active setter
                                if !is_active {
                                    if ui.button("☆").on_hover_text("Set as Default Active Profile").clicked() {
                                        self.config.active_profile = p_name.clone();
                                        self.is_dirty = true;
                                    }
                                }
                            });
                        });
                    }
                });

                // Deletion logic safely run outside loop
                if let Some(p_name) = to_delete_profile {
                    self.config.profiles.remove(&p_name);
                    if self.config.active_profile == p_name {
                        self.config.active_profile = self.config.profiles.keys().next().cloned().unwrap_or_default();
                    }
                    if self.selected_profile == Some(p_name) {
                        self.selected_profile = self.config.profiles.keys().next().cloned();
                    }
                    self.is_dirty = true;
                }

                ui.separator();

                // Create New Profile
                if self.show_new_profile_dialog {
                    ui.vertical(|ui| {
                        ui.label("Profile Name:");
                        ui.text_edit_singleline(&mut self.new_profile_name);
                        ui.horizontal(|ui| {
                            if ui.button("Create").clicked() {
                                let name = self.new_profile_name.trim().to_string();
                                if !name.is_empty() {
                                    self.config.profiles.entry(name.clone()).or_insert_with(Vec::new);
                                    self.selected_profile = Some(name);
                                    self.is_dirty = true;
                                }
                                self.show_new_profile_dialog = false;
                                self.new_profile_name.clear();
                            }
                            if ui.button("Cancel").clicked() {
                                self.show_new_profile_dialog = false;
                                self.new_profile_name.clear();
                            }
                        });
                    });
                } else {
                    if ui.button("➕ Create Profile").clicked() {
                        self.show_new_profile_dialog = true;
                    }
                }
            });

        // Main Editor Area
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let profile_name = match &self.selected_profile {
                Some(name) => name.clone(),
                None => {
                    ui.centered_and_justified(|ui| {
                        ui.label("Select or create a profile to get started.");
                    });
                    return;
                }
            };

            // Selected Profile Header
            ui.horizontal(|ui| {
                if self.is_renaming_profile {
                    ui.text_edit_singleline(&mut self.rename_profile_name);
                    if ui.button("Confirm").clicked() {
                        let new_name = self.rename_profile_name.trim().to_string();
                        if !new_name.is_empty() && new_name != profile_name {
                            if let Some(apps) = self.config.profiles.remove(&profile_name) {
                                self.config.profiles.insert(new_name.clone(), apps);
                                if self.config.active_profile == profile_name {
                                    self.config.active_profile = new_name.clone();
                                }
                                self.selected_profile = Some(new_name);
                                self.is_dirty = true;
                            }
                        }
                        self.is_renaming_profile = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.is_renaming_profile = false;
                    }
                } else {
                    ui.heading(&profile_name);
                    if ui.button("✏").on_hover_text("Rename Profile").clicked() {
                        self.rename_profile_name = profile_name.clone();
                        self.is_renaming_profile = true;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚡ Launch This Profile").clicked() {
                        if let Some(apps) = self.config.profiles.get(&profile_name) {
                            let apps_clone = apps.clone();
                            std::thread::spawn(move || {
                                let _ = launch_profile(&apps_clone);
                            });
                        }
                    }
                });
            });

            ui.separator();

            // App list
            let mut to_move_up = None;
            let mut to_move_down = None;
            let mut to_remove = None;

            if let Some(apps) = self.config.profiles.get_mut(&profile_name) {
                if apps.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label("No applications configured in this profile.");
                    });
                } else {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (idx, app) in apps.iter_mut().enumerate() {
                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    // Status check
                                    let running = *self.running_status.get(&app.desktop).unwrap_or(&false);
                                    let status_color = if running {
                                        egui::Color32::from_rgb(46, 204, 113) // Green
                                    } else {
                                        egui::Color32::from_rgb(127, 140, 141) // Gray
                                    };
                                    let status_text = if running { "RUNNING" } else { "STOPPED" };
                                    
                                    ui.colored_label(status_color, format!("● {}", status_text));

                                    // Find app display name
                                    let display_name = self.all_desktop_apps.iter()
                                        .find(|e| e.filename == app.desktop)
                                        .map(|e| e.name.clone())
                                        .unwrap_or_else(|| app.desktop.clone());

                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(display_name).strong());
                                        ui.label(egui::RichText::new(&app.desktop).small().weak());
                                    });

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        // Delete app
                                        if ui.button("❌").on_hover_text("Remove from Profile").clicked() {
                                            to_remove = Some(idx);
                                        }

                                        // Move Down
                                        if ui.button("▼").clicked() {
                                            to_move_down = Some(idx);
                                        }

                                        // Move Up
                                        if ui.button("▲").clicked() {
                                            to_move_up = Some(idx);
                                        }

                                        // Silent toggle
                                        let mut is_silent = app.silent.unwrap_or(false);
                                        if ui.checkbox(&mut is_silent, "Hide / Silent Run").changed() {
                                            app.silent = Some(is_silent);
                                            self.is_dirty = true;
                                        }

                                        // Target workspace
                                        ui.horizontal(|ui| {
                                            ui.label("Workspace:");
                                            let mut ws_val = app.workspace.clone().unwrap_or_default();
                                            let text_resp = ui.add(egui::TextEdit::singleline(&mut ws_val).desired_width(50.0));
                                            if text_resp.changed() {
                                                app.workspace = if ws_val.trim().is_empty() {
                                                    None
                                                } else {
                                                    Some(ws_val.trim().to_string())
                                                };
                                                self.is_dirty = true;
                                            }
                                        });

                                        // Immediate test run button
                                        if ui.button("▶ Run").on_hover_text("Launch this app immediately").clicked() {
                                            let app_clone = app.clone();
                                            std::thread::spawn(move || {
                                                let _ = launch_app_now(&app_clone);
                                            });
                                        }
                                    });
                                });
                            });
                            ui.add_space(4.0);
                        }
                    });
                }

                // Apply reordering/deletions
                if let Some(idx) = to_move_up {
                    if idx > 0 {
                        apps.swap(idx, idx - 1);
                        self.is_dirty = true;
                    }
                }
                if let Some(idx) = to_move_down {
                    if idx < apps.len() - 1 {
                        apps.swap(idx, idx + 1);
                        self.is_dirty = true;
                    }
                }
                if let Some(idx) = to_remove {
                    apps.remove(idx);
                    self.is_dirty = true;
                }
            }

            ui.add_space(8.0);

            // Add Application Search Panel
            if self.show_add_app_search {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Add Application").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Close").clicked() {
                                self.show_add_app_search = false;
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Search:");
                        ui.text_edit_singleline(&mut self.search_query);
                    });
                    ui.add_space(4.0);

                    let query = self.search_query.to_lowercase();
                    egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                        for entry in &self.all_desktop_apps {
                            if entry.name.to_lowercase().contains(&query) || entry.filename.to_lowercase().contains(&query) {
                                ui.horizontal(|ui| {
                                    if ui.button("➕ Add").clicked() {
                                        if let Some(apps) = self.config.profiles.get_mut(&profile_name) {
                                            apps.push(ProfileApp {
                                                desktop: entry.filename.clone(),
                                                workspace: None,
                                                silent: Some(false),
                                            });
                                            self.is_dirty = true;
                                        }
                                    }
                                    ui.label(egui::RichText::new(&entry.name).strong());
                                    ui.label(format!("({})", entry.filename));
                                    ui.weak(entry.file_path.to_string_lossy());
                                });
                            }
                        }
                    });
                });
            } else {
                if ui.button("➕ Add Application").clicked() {
                    self.show_add_app_search = true;
                }
            }
        });
    }
}

pub fn run_gui() -> Result<(), Box<dyn std::error::Error>> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("hyprlaunch - Session Manager")
            .with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    let app = HyprlaunchApp::new()?;

    eframe::run_native(
        "hyprlaunch",
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    ).map_err(|e| format!("Failed to run eframe: {:?}", e).into())
}
