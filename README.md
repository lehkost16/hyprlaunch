# hyprlaunch

`hyprlaunch` is a terminal-based **Workflow Orchestrator** for the [Hyprland](https://hyprland.org/) window manager. It provides an interactive Terminal User Interface (TUI) to define, configure, and reorder structured sequences of steps ("workflows") that you want to execute together.

Unlike state-restoring session managers, `hyprlaunch` lets you manually organize workspace mapping rules, custom shell scripts/commands, notification alerts, and delay sequences for your favorite environments (e.g., *Deep Coding*, *Content Creation*, *Meeting Prep*, or *Default* workspace setup) and launch them sequentially.

---

## Features

- **Interactive TUI Dashboard**: Keyboard-driven sidebar and editor panes to manage workflows and re-arrange execution order.
- **Sequential Execution Pipeline**: Supports four distinct step types:
  1. **Launch App**: Starts a `.desktop` application or raw binary.
  2. **Run Script**: Executes shell commands and scripts in optional target directories.
  3. **Wait**: Halts execution for a specified duration in milliseconds.
  4. **Notify**: Triggers a desktop notification to indicate progress or completions.
- **Dynamic Monitor Workspace Conditions**: Target workspaces based on connected monitors using the `HDMI-A-1?3:1` syntax (e.g., if monitor `HDMI-A-1` is connected, place the window on workspace `3`, otherwise fall back to workspace `1`).
- **Silent Launching**: Use the Hyprland `silent` execution flag to load applications in the background without stealing active window focus.
- **Integrated Health Check Scanner**: Press `v` to audit workflows for missing desktop applications, invalid script paths, excessive wait delays, and empty notification headers.
- **Automatic Legacy Migration**: Automatically upgrades old profile-based configurations (`~/.config/hyprlaunch/config.json`) to the workflows format on startup.

---

## Installation

### Prerequisites
Make sure you have Rust and Cargo installed. You also need to be running a Hyprland session to dispatch the launches.

### Build from Source
Clone the repository and build the binary:
```bash
git clone https://github.com/lehkost16/hyprlaunch.git
cd hyprlaunch
cargo build --release
```

The compiled binary will be located at `target/release/hyprlaunch`. You can move it to a folder in your `PATH` (such as `/usr/local/bin` or `~/.local/bin/`).

---

## Usage

`hyprlaunch` behaves contextually depending on how it is executed:

### Interactive mode (TUI)
Run without arguments inside a terminal to launch the setup and management TUI:
```bash
hyprlaunch
```
Alternatively, you can force the TUI to open:
```bash
hyprlaunch tui
```

#### TUI Keyboard Shortcuts
- **Workflow Navigation (Left Pane)**:
  - `Up` / `Down` or `k` / `j`: Navigate workflows list.
  - `Space`: Set the selected workflow as the default active startup workflow (indicated by a `★`).
  - `Enter`: Save and launch the selected workflow immediately (exits TUI).
  - `c`: Create a new workflow.
  - `r`: Rename the selected workflow.
  - `y`: Duplicate/Clone the selected workflow.
  - `d`: Delete the selected workflow.
  - `v`: Run health audit checks on the selected workflow steps.
  - `Tab` / `Right` / `l`: Switch pane to edit steps in the selected workflow.
  - `q` / `Esc`: Quit the application.
- **Step Navigation & Editing (Right Pane)**:
  - `Tab` / `Left` / `h`: Switch back to the workflow list.
  - `a`: Add a new step to the workflow (opens a selector popup for step types).
  - `e`: Edit all parameters of the selected step (opens a multi-field editor).
  - `d` / `Delete`: Remove the selected step.
  - `Enter`: Test launch / run only the selected step immediately in the background.
  - `Shift+Up` / `Shift+Down`: Move the selected step up or down in the startup sequence.

---

### Non-Interactive mode
If executed without arguments in a non-interactive environment (such as when called from `hyprland.conf`), `hyprlaunch` will automatically launch the **currently active workflow** (marked with `★`) and then exit.

#### Launching specific workflows via CLI
You can launch a specific workflow by name directly from your terminal or shell scripts:
```bash
hyprlaunch <workflow_name>
```

#### List workflows
Print all configured workflows (the active workflow is highlighted):
```bash
hyprlaunch list
```

---

## Configuration File

Your workflows and settings are stored in JSON format at:
`~/.config/hyprlaunch/config.json`

### Environment Variable Override
You can override the default config location by setting the `HYPRLAUNCH_CONFIG` environment variable:
```bash
export HYPRLAUNCH_CONFIG="$HOME/custom/path/config.json"
```

### Config Schema Example
Here is how your `config.json` might look:
```json
{
  "active_workflow": "coding",
  "workflows": {
    "coding": {
      "name": "coding",
      "steps": [
        {
          "type": "notify",
          "title": "Developer Workspace",
          "body": "Booting development environment..."
        },
        {
          "type": "launch",
          "desktop": "firefox.desktop",
          "workspace": "1",
          "silent": false
        },
        {
          "type": "script",
          "command": "git pull",
          "dir": "/home/user/projects/hyprlaunch"
        },
        {
          "type": "wait",
          "ms": 500
        },
        {
          "type": "launch",
          "desktop": "kitty.desktop",
          "workspace": "2",
          "silent": true,
          "monitor_cond": "HDMI-A-1?3:2"
        }
      ]
    }
  }
}
```

---

## Integration with Hyprland

To automatically launch your active workflow when you log into Hyprland, add the following line to your `~/.config/hypr/hyprland.conf`:

```ini
exec-once = hyprlaunch
```

You can also create keybindings to easily switch workflows:

```ini
# Switch to and launch the "gaming" workflow
bind = $mainMod, G, exec, hyprlaunch gaming

# Switch to and launch the "coding" workflow
bind = $mainMod, C, exec, hyprlaunch coding
```

---

## License

This project is licensed under the GPL-3.0 License.