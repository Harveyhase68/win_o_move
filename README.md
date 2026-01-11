# WinOMove

A lightweight Windows tray application that restores the `Win+Shift+Left/Right` keyboard shortcuts for moving windows between monitors - even when Windows Snap Assist is disabled.

## Why?

Many users disable Windows Snap Assist (System → Multitasking → Snap windows) because of the annoying window suggestions that pop up. Unfortunately, this also disables the useful `Win+Shift+Arrow` shortcuts for moving windows between monitors.

**WinOMove solves this problem** by providing just the monitor-switching functionality without any of the Snap Assist bloat.

## Features

- Move the active window to the next monitor with `Win+Shift+Left` or `Win+Shift+Right`
- Lightweight tray application (~800KB)
- No configuration needed - just start and use
- Clean exit via tray icon menu
- Preserves window position relative to the monitor

## Installation

### Option 1: Download Release
Download the latest `win_o_move.exe` from the [Releases](https://github.com/Harveyhase68/win_o_move/releases) page.

### Option 2: Build from Source

**Prerequisites:**
- [Rust](https://rustup.rs/) (1.70 or newer)
- Windows 10/11

**Build:**
```bash
git clone https://github.com/Harveyhase68/win_o_move.git
cd win_o_move
cargo build --release
```

The executable will be at `target/release/win_o_move.exe`.

## Usage

1. **Start** the application by double-clicking `win_o_move.exe`
2. A blue icon appears in the system tray
3. **Use the hotkeys:**
   - `Win+Shift+Left` - Move active window to the monitor on the left
   - `Win+Shift+Right` - Move active window to the monitor on the right
4. **Exit** by right-clicking the tray icon and selecting "Beenden"

### Autostart (Optional)

To start WinOMove automatically with Windows:

1. Press `Win+R` and type `shell:startup`
2. Copy `win_o_move.exe` (or create a shortcut) into this folder

## How It Works

WinOMove uses a low-level keyboard hook to detect the `Win+Shift+Arrow` key combinations. When triggered, it:

1. Gets the currently active (foreground) window
2. Determines which monitor the window is on
3. Finds the adjacent monitor in the requested direction
4. Moves the window to the same relative position on the target monitor
5. Forces a redraw to ensure proper rendering

## Future Ideas

- Configurable hotkeys
- Maximize/restore window shortcuts
- Settings file for customization
- Multi-language support

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

---

Made with Rust and a little help from Claude.
