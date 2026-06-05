# hypr-quicklaunch

`hypr-quicklaunch` is a simple, profile-based application launcher for the [Hyprland](https://hyprland.org/) window manager. It provides an interactive terminal user interface (TUI) to define, configure, and reorder groups of applications ("profiles") that you want to launch together.

Unlike state-restoring session managers, `hypr-quicklaunch` lets you manually organize workspace mappings, startup delays, and focus settings for your favorite environments (e.g., *Work*, *Development*, *Gaming*, or *Default* startup apps).

---

## Features

- **Interactive TUI Configurator**: Quickly create and edit profiles and associate them with applications.
- **Auto-Detect System Desktop Entries**: Scans your local and system applications (`/usr/share/applications` and `~/.local/share/applications`), offering search-as-you-type to easily add programs.
- **Workspace Mapping**: Assign applications to launch on specific Hyprland workspaces (e.g. `1`, `2`, or `silent`).
- **Launch Delays**: Configure specific delays in milliseconds (`delay_ms`) between application launches to avoid race conditions and resource spikes.
- **Silent Launching**: Use the Hyprland `silent` execution flag to load applications in the background without stealing active window focus.
- **Startup Order Control**: Reorder execution sequences directly inside the TUI.
- **Flexible Launching**: Seamlessly integrates with Hyprland's startup config or keybindings.

---

## Installation

### Prerequisites
Make sure you have Rust and Cargo installed. You also need to be running a Hyprland session to dispatch the launches.

### Build from Source
Clone the repository and build the binary:
```bash
git clone https://github.com/lehkost16/hypr-quicklaunch.git
cd hypr-quicklaunch
cargo build --release
```

The compiled binary will be located at `target/release/hypr-quicklaunch`. You can move it to a folder in your `PATH` (such as `/usr/local/bin` or `~/.local/bin/`).

---

## Usage

`hypr-quicklaunch` behaves contextually depending on how it is executed:

### Interactive mode (TUI)
Run without arguments inside a terminal to launch the setup and management TUI:
```bash
hypr-quicklaunch
```
Alternatively, you can force the TUI to open:
```bash
hypr-quicklaunch tui
```

#### TUI Keyboard Shortcuts
- **Profile Navigation**:
  - `Up` / `Down` or `k` / `j`: Navigate profiles list
  - `Enter`: Set the selected profile as active and launch it
  - `c`: Create a new profile
  - `d`: Delete the selected profile
  - `Tab` / `Right` / `l`: Switch pane to edit applications in the selected profile
  - `q` / `Esc`: Quit the application
- **Application Navigation (Right Pane)**:
  - `Tab` / `Left` / `h`: Switch back to the profile list
  - `a`: Add a new application to the profile (opens a search overlay of system applications)
  - `d` / `Delete`: Remove the selected application from the profile
  - `w`: Set targeted workspace rule (leave empty for default workspace behavior)
  - `t`: Set startup delay in milliseconds (e.g., `500` ms)
  - `s`: Toggle launcher silent execution (`[Silent]` flag)
  - `Shift+Up` / `Shift+Down`: Move the selected application up or down in the startup sequence

---

### Non-Interactive mode
If executed without arguments in a non-interactive environment (such as when called from `hyprland.conf`), `hypr-quicklaunch` will automatically launch all applications registered under the **currently active profile** and then exit.

#### Launching specific profiles via CLI
You can launch a specific profile by name directly from your terminal or shell scripts:
```bash
hypr-quicklaunch <profile_name>
```

#### List profiles
Print all configured profiles (the active profile is highlighted):
```bash
hypr-quicklaunch list
```

---

## Configuration File

Your profiles and settings are stored in JSON format at:
`~/.config/hypr-quicklaunch/config.json`

### Environment Variable Override
You can override the default config location by setting the `HYPR_QUICKLAUNCH_CONFIG` environment variable:
```bash
export HYPR_QUICKLAUNCH_CONFIG="$HOME/custom/path/config.json"
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
        "silent": false,
        "delay_ms": 0
      },
      {
        "desktop": "kitty.desktop",
        "workspace": "2",
        "silent": true,
        "delay_ms": 300
      }
    ],
    "work": [
      {
        "desktop": "slack.desktop",
        "workspace": "3",
        "silent": false,
        "delay_ms": 500
      },
      {
        "desktop": "org.codeberg.dnkl.foot.desktop",
        "workspace": "1",
        "silent": false,
        "delay_ms": 0
      }
    ]
  }
}
```

---

## Integration with Hyprland

To automatically launch your active profile when you log into Hyprland, add the following line to your `~/.config/hypr/hyprland.conf`:

```ini
exec-once = hypr-quicklaunch
```

You can also create keybindings to easily switch profiles:

```ini
# Switch to and launch the "gaming" profile
bind = $mainMod, G, exec, hypr-quicklaunch gaming

# Switch to and launch the "work" profile
bind = $mainMod, W, exec, hypr-quicklaunch work
```

---

## License

This project is licensed under the GPL-3.0 License.