use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopEntry {
    pub filename: String,
    pub file_path: std::path::PathBuf,
    pub name: String,
    pub exec: Vec<String>,
    pub path: Option<String>,
    pub hidden: bool,
    pub terminal: bool,
}

pub fn get_desktop_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // 1. User applications: $XDG_DATA_HOME/applications or ~/.local/share/applications
    if let Ok(val) = std::env::var("XDG_DATA_HOME") {
        if !val.is_empty() {
            paths.push(Path::new(&val).join("applications"));
        }
    } else if let Ok(home) = std::env::var("HOME") {
        paths.push(Path::new(&home).join(".local/share/applications"));
    }

    // 2. System applications: $XDG_DATA_DIRS/applications or standard dirs
    if let Ok(val) = std::env::var("XDG_DATA_DIRS") {
        for dir in val.split(':') {
            if !dir.is_empty() {
                paths.push(Path::new(dir).join("applications"));
            }
        }
    } else {
        paths.push(PathBuf::from("/usr/local/share/applications"));
        paths.push(PathBuf::from("/usr/share/applications"));
    }

    paths
}

pub fn scan_desktop_entries() -> Vec<DesktopEntry> {
    let search_paths = get_desktop_search_paths();
    let mut entries = Vec::new();
    let mut seen_filenames = HashSet::new();

    for path in search_paths {
        if !path.is_dir() {
            continue;
        }
        if let Ok(read_dir) = std::fs::read_dir(path) {
            for entry in read_dir.flatten() {
                let file_path = entry.path();
                if file_path.is_file() {
                    if let Some(ext) = file_path.extension() {
                        if ext == "desktop" {
                            if let Some(filename) = file_path.file_name().and_then(|f| f.to_str()) {
                                let filename_string = filename.to_string();
                                if seen_filenames.contains(&filename_string) {
                                    continue;
                                }
                                seen_filenames.insert(filename_string.clone());

                                if let Some(desktop_entry) = parse_desktop_file(&file_path) {
                                    if !desktop_entry.hidden {
                                        entries.push(desktop_entry);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort alphabetically by name
    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries
}

pub fn find_desktop_file(name: &str) -> Option<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() && path.exists() {
        return Some(path.to_path_buf());
    }

    let search_paths = get_desktop_search_paths();
    let target_filename = if name.ends_with(".desktop") {
        name.to_string()
    } else {
        format!("{}.desktop", name)
    };

    for dir in search_paths {
        let file_path = dir.join(&target_filename);
        if file_path.is_file() {
            return Some(file_path);
        }
    }

    None
}

pub fn parse_desktop_file(file_path: &Path) -> Option<DesktopEntry> {
    let file = File::open(file_path).ok()?;
    let reader = BufReader::new(file);
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut path = None;
    let mut no_display = false;
    let mut hidden = false;
    let mut terminal = false;

    let filename = file_path.file_name()?.to_string_lossy().into_owned();

    for line in reader.lines() {
        let line = line.ok()?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if trimmed == "[Desktop Entry]" {
                in_desktop_entry = true;
            } else {
                in_desktop_entry = false;
            }
            continue;
        }

        if in_desktop_entry {
            if let Some(pos) = trimmed.find('=') {
                let key = trimmed[..pos].trim();
                let val = trimmed[pos + 1..].trim();

                if key == "Name" {
                    name = Some(val.to_string());
                } else if key == "Exec" {
                    exec = Some(val.to_string());
                } else if key == "Path" {
                    path = Some(val.to_string());
                } else if key == "NoDisplay" {
                    no_display = val.to_lowercase() == "true";
                } else if key == "Hidden" {
                    hidden = val.to_lowercase() == "true";
                } else if key == "Terminal" {
                    terminal = val.to_lowercase() == "true";
                }
            }
        }
    }

    let exec_str = exec?;
    let exec_args = parse_exec_line(&exec_str)?;
    let name_str = name.unwrap_or_else(|| {
        filename.strip_suffix(".desktop").unwrap_or(&filename).to_string()
    });

    Some(DesktopEntry {
        filename,
        file_path: file_path.to_path_buf(),
        name: name_str,
        exec: exec_args,
        path: path.filter(|s| !s.is_empty()),
        hidden: no_display || hidden,
        terminal,
    })
}

pub fn parse_exec_line(cmd_line: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let mut escaped = false;

    for c in cmd_line.chars() {
        if escaped {
            current.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
        } else if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
        } else if c.is_whitespace() && !in_double_quote && !in_single_quote {
            if !current.is_empty() {
                args.push(current.clone());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        args.push(current);
    }

    let placeholders = ["%f", "%F", "%u", "%U", "%d", "%D", "%n", "%N", "%i", "%c", "%k"];
    let filtered_args: Vec<String> = args
        .into_iter()
        .filter(|arg| !placeholders.contains(&arg.as_str()))
        .collect();

    if filtered_args.is_empty() {
        None
    } else {
        Some(filtered_args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_exec_line() {
        assert_eq!(
            parse_exec_line("firefox %u").unwrap(),
            vec!["firefox".to_string()]
        );
        assert_eq!(
            parse_exec_line("steam -tony %U").unwrap(),
            vec!["steam".to_string(), "-tony".to_string()]
        );
        assert_eq!(
            parse_exec_line("kitty -e \"tmux new-session\"").unwrap(),
            vec!["kitty".to_string(), "-e".to_string(), "tmux new-session".to_string()]
        );
        assert_eq!(
            parse_exec_line("cmd \\\"escaped\\\"").unwrap(),
            vec!["cmd".to_string(), "\"escaped\"".to_string()]
        );
    }

    #[test]
    fn test_parse_desktop_file_terminal() {
        use std::io::Write;
        let dir = Path::new("target");
        let file_path = dir.join("test_term.desktop");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "[Desktop Entry]").unwrap();
        writeln!(file, "Name=Test App").unwrap();
        writeln!(file, "Exec=btop").unwrap();
        writeln!(file, "Terminal=true").unwrap();
        drop(file);

        let entry = parse_desktop_file(&file_path).unwrap();
        assert_eq!(entry.name, "Test App");
        assert_eq!(entry.exec, vec!["btop".to_string()]);
        assert!(entry.terminal);

        std::fs::remove_file(file_path).ok();
    }
}
