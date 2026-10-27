# hyprlaunch

`hyprlaunch` is a terminal-based **Workflow Orchestrator** for the [Hyprland](https://hyprland.org/) window manager. It provides an interactive Terminal User Interface (TUI) to define, configure, and reorder structured sequences of steps ("workflows") that you want to execute together.

Unlike state-restoring session managers, `hyprlaunch` lets you organize workspace mapping rules, scripts, notifications, and readiness checks into reusable workflows. Steps stay ordered by default, with optional parallel app launches for independent startup tasks.

---

## Features

- **Interactive TUI Dashboard**: Keyboard-driven sidebar and editor panes to manage workflows and re-arrange execution order.
- **Workflow Execution Pipeline**: Supports six step types and explicit ordering:
  1. **Launch App**: Starts a `.desktop` application or raw binary.
  2. **Run Script**: Executes shell commands and scripts in optional target directories.
  3. **Wait**: Halts execution for a specified duration in milliseconds.
  4. **Wait for Window**: Continues when a matching Hyprland window class appears, with a timeout.
  5. **Notify**: Triggers a desktop notification to indicate progress or completions.
  6. **Dispatch**: Sends a command to Hyprland.
- **Parallel App Launch**: Consecutive app-launch steps can start together; other step types remain ordered barriers.
- **Failure Policy**: Choose whether a workflow continues after a failed step or skips the remaining steps.
- **Dynamic Monitor Workspace Conditions**: Target workspaces based on connected monitors using the `HDMI-A-1?3:1` syntax (e.g., if monitor `HDMI-A-1` is connected, place the window on workspace `3`, otherwise fall back to workspace `1`).
- **Silent Launching**: Use the Hyprland `silent` execution flag to load applications in the background without stealing active window focus.
- **Integrated Health Check Scanner**: Press `v` to audit workflows for missing desktop applications, invalid script paths, excessive wait delays, invalid window waits, and empty notification headers.

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
  - `Enter`: Save and run the selected workflow; progress and step results stay visible in the TUI.
  - `c`: Create a new workflow.
  - `r`: Rename the selected workflow.
  - `y`: Duplicate/Clone the selected workflow.
  - `d`: Delete the selected workflow.
  - `v`: Run health audit checks on the selected workflow steps.
  - `p`: Toggle the selected workflow's failure policy between Continue and Stop.
  - `Tab` / `Right` / `l`: Switch pane to edit steps in the selected workflow.
  - `q` / `Esc`: Quit the application.
- **Step Navigation & Editing (Right Pane)**:
  - `Tab` / `Left` / `h`: Switch back to the workflow list.
  - `a`: Add a new step to the workflow (opens a selector; press `1`–`7` to choose a type directly).
  - Application search accepts partial names and fuzzy character matches, ranked by relevance.
  - `e`: Edit all parameters of the selected step (opens a multi-field editor).
  - `y`: Duplicate the selected step directly below it.
  - `d` / `Delete`: Remove the selected step.
  - `Enter`: Test launch / run only the selected step immediately in the background.
  - `Shift+Up` / `Shift+Down`: Move the selected step up or down in the startup sequence.
  - In a Launch step editor, `Ctrl+P` toggles parallel launch. Adjacent Launch steps with this enabled start together.

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
      "on_error": "continue",
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
          "silent": false,
          "parallel": true
        },
        {
          "type": "launch",
          "desktop": "kitty.desktop",
          "workspace": "2",
          "silent": true,
          "monitor_cond": "HDMI-A-1?3:2",
          "parallel": true
        },
        {
          "type": "wait_window",
          "class": "kitty",
          "timeout_ms": 15000
        },
        {
          "type": "script",
          "command": "git pull",
          "dir": "/home/user/projects/hyprlaunch"
        }
      ]
    }
  }
}
```

Every workflow must set `on_error`, and every launch step must set `parallel`. Set `on_error` to `"stop"` to skip subsequent steps after a failure, or `"continue"` to keep running them. Consecutive launch steps with `parallel: true` are dispatched concurrently; use `false` for sequential launches. If a parallel step fails under the Stop policy, its already-started peers finish before later steps are skipped. Use `wait_window` after launching an app when a later action depends on its window being ready, instead of guessing with a fixed delay. The loader reads only the current workflow schema; it does not migrate older profile-format files.

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
