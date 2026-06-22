# replayd

**Capture your favorite moments, focused around gaming, on Linux.**

`replayd` is a native Linux instant-replay application built with GTK4 and libadwaita. It runs in the background (typically minimized to the background or tray rather than as a separate system service), continuously buffering your screen, and lets you save the last few seconds (or minutes) of gameplay with a single global shortcut — no need to keep OBS running or sacrifice performance with always-on recording. Saved clips land in a built-in library where you can browse, filter, and favorite them.

The actual screen/audio capture is performed by [gpu-screen-recorder](https://git.dec05eba.com/gpu-screen-recorder/about/), a GPU-accelerated screen recorder for Linux. replayd drives it under the hood to implement the rolling buffer and clip-saving behavior, and wraps it with a GTK4 GUI, a clip library, and desktop-native global shortcut integration.

> Primary repository: [codeberg.org/Land/replayd](https://codeberg.org/Land/replayd)

> Mirror: [github.com/LandStander27/replayd](https://github.com/LandStander27/replayd)

---

## Table of Contents

- [Features](#features)
- [How It Works](#how-it-works)
- [Project Layout](#project-layout)
- [Installation](#installation)
    - [Dependencies](#dependencies)
    - [Building From Source](#building-from-source)
    - [Installing](#installing)
    - [Arch Linux Package](#arch-linux-package)
- [Usage](#usage)
    - [Running replayd](#running-replayd)
    - [Setting the Global Shortcut](#setting-the-global-shortcut)
    - [Controlling replayd from the Command Line](#controlling-replayd-from-the-command-line)
    - [The Clip Library](#the-clip-library)
- [Configuration](#configuration)
- [Uninstalling](#uninstalling)
- [Troubleshooting](#troubleshooting)
- [License](#license)

---

## Features

- **Background instant replay** — `replayd` continuously buffers your screen while running in the background so you never miss a clutch, a funny bug, or a rage-quit-worthy death.
- **GPU-accelerated capture** — Recording itself is handled by [`gpu-screen-recorder`](https://git.dec05eba.com/gpu-screen-recorder/about/), which `replayd` runs and controls under the hood. This keeps capture overhead low and lets you take advantage of your GPU's hardware encoder.
- **Global shortcut clipping, the desktop-native way** — Save the last N seconds of buffered footage on demand, using a shortcut registered through the `xdg-desktop-portal` **GlobalShortcuts** portal. This means `replayd` doesn't read raw input devices or need special permissions to catch your hotkey — it works through whatever portal backend your desktop/compositor provides, and the shortcut itself is configured the same way as any other portal-based shortcut on your system.
- **Native clip library GUI** — Browse saved clips from within the app, complete with:
    - Per-clip properties (duration, game, date, etc.)
    - Filtering clips by game
    - Favoriting clips you want to keep close at hand
- **Shell scripting** — `replaydctl` is a command-line tool for interacting with a running `replayd` instance, useful for scripting.
- **Configurable buffer length** — Tune how much footage is kept in the rolling buffer (in seconds) to balance memory/disk usage against how far back you can clip.
- **Portable clip storage** — Clip paths are stored relative to the library location, so your clip library can be moved or backed up without breaking references.

## How It Works

`replayd` follows the same idea as proprietary tools like NVIDIA ShadowPlay's Instant Replay or Medal, brought natively to Linux desktops:

1. `replayd` runs continuously in the background, using `gpu-screen-recorder` to capture and buffer your screen into a rolling window of footage.
2. A global shortcut — registered via the `xdg-desktop-portal` GlobalShortcuts portal, rather than a raw input hook — tells `replayd` to flush the requested portion of that buffer to disk as a saved clip.
3. The same application doubles as your **clip library**: you can preview, organize, tag games, favorite, and inspect clip properties without any separate tooling.
4. Whenever you save a clip, `replayd` will attempt to identify what game you are currently playing by looking at the active window. The games that can be identified are located [here](https://cdn.landsj.dev/games.jsonc), which is fetched and parsed at runtime. Because of Wayland limitations, the method of getting the active window is different for every window manager, thus only certain window managers are supported. The currently supported ones are located [here](./src/identifier.rs), within the function `get_window_manager()`. If yours is not supported, either open an issue or submit a PR.

Because capture happens continuously in a fixed-duration buffer rather than recording every session in full, disk and resource usage stay small regardless of how long you play. Relying on the portal for shortcuts also means `replayd` plays nicely across different desktops and compositors without needing elevated permissions just to listen for a keypress, and offloading the actual encode to `gpu-screen-recorder` keeps the capture path GPU-accelerated and lightweight.

## Project Layout

```
replayd/
├── .forgejo/workflows/   # CI configuration (Forgejo Actions)
├── assets/               # Icons, .desktop files, and other install assets
├── hooks/                # Git hooks to ensure quality when making changes
├── src/                  # Rust source for replayd, replayd-hook, and replaydctl
├── version/              # Auto versioning crate
├── Cargo.toml            # Workspace manifest
├── README.md             # Me
└── LICENSE               # MIT license
```

> `replayd` is written entirely in Rust.

## Installation

### Dependencies

Because `replayd` uses GTK4 and libadwaita for its UI and relies on `xdg-desktop-portal` for global shortcuts, you'll need development libraries for those, plus the Rust toolchain.

On **Arch Linux**:

```bash
sudo pacman -S --needed \
    hicolor-icon-theme glib2 glibc cairo \
    gtk4 libadwaita alsa-lib gstreamer \
    libgcc libxcb openssl gpu-screen-recorder \
    git rust
```

On **Debian/Ubuntu** (package names approximate — verify against your distro's repositories, and build/install [`gpu-screen-recorder`](https://git.dec05eba.com/gpu-screen-recorder/about/) separately, as it isn't packaged everywhere):

```bash
sudo apt install \
    hicolor-icon-theme libglib2.0-dev libgtk-4-dev libadwaita-1-dev \
    libcairo2-dev libasound2-dev libgstreamer1.0-dev \
    libgcc-s1 libxcb1-dev libssl-dev \
    git build-essential pkg-config libgit2-dev sed \
    gpu-screen-recorder
sudo apt install rustc cargo   # or install Rust via https://rustup.rs
```

You'll also need a working `xdg-desktop-portal` GlobalShortcuts backend appropriate for your desktop/compositor (e.g. `xdg-desktop-portal-gnome`, `xdg-desktop-portal-kde`, `xdg-desktop-portal-wlr`, or `xdg-desktop-portal-hyprland`) for the global shortcut to register — GNOME and KDE Plasma ship one out of the box, while other (especially wlroots-based) compositors typically need one installed and configured separately. This isn't listed as a hard package dependency since most desktop environments already provide a suitable portal implementation.

### Building From Source

```bash
# Clone the repository
git clone https://codeberg.org/Land/replayd.git
cd replayd

# Build replayd, replayd-hook, and replaydctl together
cargo build --release --features socket_commands --locked
```

This produces three binaries under `target/release/`:

- **`replayd`** — the main application: GUI, clip library, and instant-replay engine.
- **`replayd-hook`** — a small companion binary needed as a callback for `gpu-screen-recorder`, so `replayd` can index the clips saved.
- **`replaydctl`** — a command-line control tool for talking to a running `replayd` instance over a local socket.
  The `socket_commands` feature enables the socket-based control interface that `replaydctl` both relies on. If you only want the GUI application and don't need scripting/hook support, you can omit it:

```bash
cargo build --release --locked
```

> If you'd rather not build manually, see [Arch Linux Package](#arch-linux-package) below for a `makepkg`-based install.

### Installing

```bash
# Install the binaries
sudo install -Dm755 "target/release/replayd" "/usr/bin/replayd"
sudo install -Dm755 "target/release/replayd-hook" "/usr/bin/replayd-hook"
sudo install -Dm755 "target/release/replaydctl" "/usr/bin/replaydctl" # If you built with --features socket_commands

# Install license, icon, and desktop entry
sudo install -Dm644 "LICENSE" -t "/usr/share/licenses/replayd/"
sudo install -Dm644 "assets/icon.svg" -T "/usr/share/icons/hicolor/scalable/apps/dev.land.Replayd.svg"
sudo install -Dm644 "assets/dev.land.Replayd.desktop" -t "/usr/share/applications/"
```

> Because `replayd` is the application itself (not a separate background service), there's no `systemd` unit to install — just launch it like any other desktop app, and optionally add it to your session autostart if you want it running as soon as you log in.

### Arch Linux Package

If you're on Arch Linux (or an Arch-based distro), a `replayd-git` package is available from a third-party repository, **landware**, hosted by the project's maintainer.

Add the following to `/etc/pacman.conf`:

```ini
[landware]
Server = https://repo.landsj.dev/landware/x86_64
SigLevel = DatabaseNever PackageNever TrustedOnly
```

Then sync and install:

```bash
sudo pacman -Sy
sudo pacman -S replayd-git
```

> **⚠️ Security warning:** Adding a third-party repo is inherently riskier than installing from the official Arch repositories or building from source yourself. Only add this repository if you trust the project's maintainer and are comfortable with that trade-off. If you'd rather not take on that risk, build from source instead using the steps above. The package is automatically updating, using the `PKGBUILD` located [here](https://codeberg.org/Land/landware/src/branch/master/replayd-git/PKGBUILD).

## Usage

### Running replayd

Launch it from your application launcher, or directly from a terminal:

```bash
replayd
```

To have it running in the background automatically whenever you start a gaming session, add it to your desktop environment's autostart/startup applications (or place a `.desktop` autostart entry in `~/.config/autostart/`).

### Setting the Global Shortcut

`replayd` registers its "save clip" action as a global shortcut through the `xdg-desktop-portal` GlobalShortcuts portal. Depending on your desktop:

- **GNOME / KDE Plasma** — the shortcut typically appears in your system settings alongside other app shortcuts, or is requested/bound the first time `replayd` asks for it.
- **wlroots-based compositors (Sway, Hyprland, etc.)** — make sure a GlobalShortcuts-capable portal backend is installed and running, then bind/confirm the shortcut as prompted by `replayd` or your portal backend's configuration.

### Controlling replayd from the Command Line

If built with the `socket_commands` feature, you can interact with a running `replayd` instance from the terminal — useful for scripting:

```bash
replaydctl clip      # save the current buffer as a clip
replaydctl toggle    # toggle clipping
```

### The Clip Library

Open `replayd`'s library view to browse everything it has captured:

- **Filter by game** using the dropdown at the top of the library view.
- **Favorite** clips you want to find quickly later.
- **Inspect clip properties** (duration, capture date, associated game) from the properties dialog.

Clip file paths are stored relative to your library root, so the entire clip folder can be relocated or backed up to another drive without breaking the app's references to your clips.

## Configuration

`replayd` exposes user-tunable settings from within its own settings panel, most notably the **buffer length**, which controls how much footage is kept in memory/on disk before being overwritten (configured in seconds). Check the in-app settings for the full list of available options, including capture quality, storage location.

## Uninstalling

```bash
sudo rm /usr/bin/replayd
sudo rm -f /usr/bin/replaydctl
sudo rm -f /usr/bin/replayd-hook
sudo rm -rf /usr/share/licenses/replayd/
sudo rm -f /usr/share/icons/hicolor/scalable/apps/dev.land.Replayd.svg
sudo rm -f /usr/share/applications/dev.land.Replayd.desktop
```

If you added an autostart entry, remove it from `~/.config/autostart/` as well.

## Troubleshooting

- **Global shortcut doesn't trigger a clip** — Confirm a GlobalShortcuts-capable `xdg-desktop-portal` backend is installed and running for your desktop/compositor. On non-GNOME/KDE setups this is the most common cause of shortcuts silently not registering.
- **Library shows no clips** — Verify `replayd`'s settings point at the clip storage directory you expect, and that the directory is writable.

If you run into an issue not covered here, please open an issue on the [Codeberg issue tracker](https://codeberg.org/Land/replayd/issues).

## License

`replayd` is licensed under the **MIT License**. See [LICENSE](./LICENSE) for the full text.

---

_The only place AI was used was to help create this README, and to design the UI, as I am horrible at making things look aesthetically good. All code was written by a human._
