# hyprlaunch

`hyprlaunch` is a simple, profile-based application launcher for the [Hyprland](https://hyprland.org/) window manager. It provides an interactive terminal user interface (TUI) to define, configure, and reorder groups of applications ("profiles") that you want to launch together.

Unlike state-restoring session managers, `hyprlaunch` lets you manually organize workspace mappings, custom shell scripts/commands, and focus settings for your favorite environments (e.g., *Work*, *Development*, *Gaming*, or *Default* startup apps) and launch them concurrently.

---

## Features

- **Interactive TUI Configurator**: Quickly create, clone, rename, and edit profiles and associate them with apps.
- **Auto-Detect System Desktop Entries**: Scans your local and system applications (`/usr/share/applications` and `~/.local/share/applications`), offering search-as-you-type to easily add programs.
- **Custom Commands & Scripts Support**: Add raw shell commands (e.g. `waybar` or a custom bash script) directly to profiles instead of only `.desktop` applications.
- **Workspace Mapping**: Assign applications to launch on specific Hyprland workspaces (e.g. `1`, `2`, or `silent`).
- **Silent Launching**: Use the Hyprland `silent` execution flag to load applications in the background without stealing active window focus.
- **Startup Order Control**: Reorder execution sequences directly inside the TUI.
- **Test Launching**: Instantly test run individual applications or custom commands directly from the editor pane.
- **Active Profile Selection**: Toggle your default active startup profile directly in the TUI sidebar.
- **Flexible Launching**: Seamlessly integrates with Hyprland's startup config or keybindings.

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
- **Profile Navigation (Left Pane)**:
  - `Up` / `Down` or `k` / `j`: Navigate profiles list
  - `Space`: Set the selected profile as the default active startup profile (indicated by a `★`)
  - `Enter`: Save and launch the selected profile immediately (exits TUI)
  - `c`: Create a new profile
  - `r`: Rename the selected profile
  - `y`: Duplicate/Clone the selected profile
  - `d`: Delete the selected profile
  - `Tab` / `Right` / `l`: Switch pane to edit applications in the selected profile
  - `q` / `Esc`: Quit the application
- **Application Navigation (Right Pane)**:
  - `Tab` / `Left` / `h`: Switch back to the profile list
  - `a`: Add a new desktop application to the profile (opens a search overlay of system applications)
  - `c`: Add a custom shell command or bash script to the profile
  - `d` / `Delete`: Remove the selected application/command from the profile
  - `w`: Set targeted workspace rule (leave empty for default workspace behavior)
  - `s`: Toggle launcher silent execution (`[Silent]` flag)
  - `Enter`: Test launch only the selected application/command immediately in the background
  - `Shift+Up` / `Shift+Down`: Move the selected application up or down in the startup sequence

---

### Non-Interactive mode
If executed without arguments in a non-interactive environment (such as when called from `hyprland.conf`), `hyprlaunch` will automatically launch all applications registered under the **currently active profile** (marked with `★`) and then exit.

#### Launching specific profiles via CLI
You can launch a specific profile by name directly from your terminal or shell scripts:
```bash
hyprlaunch <profile_name>
```

#### List profiles
Print all configured profiles (the active profile is highlighted):
```bash
hyprlaunch list
```

---

## Configuration File

Your profiles and settings are stored in JSON format at:
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
  "active_profile": "work",
  "profiles": {
    "default": [
      {
        "desktop": "firefox.desktop",
        "workspace": "1",
        "silent": false
      },
      {
        "desktop": "kitty.desktop",
        "workspace": "2",
        "silent": true
      }
    ],
    "work": [
      {
        "desktop": "slack.desktop",
        "workspace": "3",
        "silent": false
      },
      {
        "desktop": "waybar",
        "workspace": null,
        "silent": false
      },
      {
        "desktop": "org.codeberg.dnkl.foot.desktop",
        "workspace": "1",
        "silent": false
      }
    ]
  }
}
```

---

## Integration with Hyprland

To automatically launch your active profile when you log into Hyprland, add the following line to your `~/.config/hypr/hyprland.conf`:

```ini
exec-once = hyprlaunch
```

You can also create keybindings to easily switch profiles:

```ini
# Switch to and launch the "gaming" profile
bind = $mainMod, G, exec, hyprlaunch gaming

# Switch to and launch the "work" profile
bind = $mainMod, W, exec, hyprlaunch work
```

---

## License

This project is licensed under the GPL-3.0 License.