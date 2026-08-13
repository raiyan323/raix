# Raix

> ⚡ A fast, minimal, keyboard-first application launcher for Wayland.

**Raix** is a lightweight application launcher written in **Rust + GTK4**, designed for Linux users who prefer fast, keyboard-driven workflows.

It uses **Wayland layer-shell** through `gtk4-layer-shell`, making it especially suitable for Wayland compositors such as **Hyprland**.

---

## ✨ Features

- ⚡ Fast application launcher
- 🔍 Fuzzy application search
- ⌨️ Keyboard-first workflow
- 🐧 Native Wayland support
- 🎯 `wlr-layer-shell` integration
- 🎨 TOML-based configuration
- 🖼️ Application icons
- 📝 Application descriptions
- 🚀 Lightweight Rust implementation
- ⬆️⬇️ Navigate results while keeping search focus
- ↵ Launch applications with Enter
- `Esc` closes the launcher
- 📦 Arch Linux / PKGBUILD support

---

## 🖼️ Screenshot

<p align="center">
  <img src="assets/screenshot.png" alt="Raix Launcher" width="850">
</p>

---

## 🧰 Requirements

- Linux
- Wayland
- A Wayland compositor
- `wlr-layer-shell` support
- Rust
- Cargo
- GTK4
- `gtk4-layer-shell`

---

## 📦 Arch Linux

Raix includes a `PKGBUILD`, so Arch users can build and install it directly.

### Install dependencies

```bash
sudo pacman -S --needed base-devel rust cargo gtk4 gtk4-layer-shell wayland wayland-protocols pkgconf
```

Depending on your system and dependencies, you may also need:

```bash
sudo pacman -S --needed pkgconf libxkbcommon
```

---

## 🚀 Build from Source

Clone the repository:

```bash
git clone https://github.com/raiyan323/raix.git
```

Enter the project directory:

```bash
cd raix
```

Build and install the Arch package:

```bash
makepkg -si
```

That's it.

After installation, run:

```bash
raix
```

---

## 🛠️ Build Without Installing

If you only want to build the package:

```bash
makepkg
```

The resulting package will appear in the current directory:

```text
raix-*.pkg.tar.zst
```

You can install it later with:

```bash
sudo pacman -U raix-*.pkg.tar.zst
```

---

# ⚙️ Configuration

On the first launch, Raix creates:

```text
~/.config/raix/config.toml
```

Edit it with your preferred editor:

```bash
nano ~/.config/raix/config.toml
```

or:

```bash
vim ~/.config/raix/config.toml
```

Example:

```toml
width = 760
height = 520

background = "rgba(20, 20, 28, 0.97)"

text_color = "#eeeeee"
comment_color = "#858591"

selected_color = "rgba(110, 140, 255, 0.22)"
hover_color = "rgba(255, 255, 255, 0.07)"
border_color = "rgba(255, 255, 255, 0.10)"

border_radius = 18
border_width = 1

font = "JetBrainsMono Nerd Font"

app_font_size = 15
comment_font_size = 11
search_font_size = 17

search_placeholder = "Search applications..."

show_icons = true
show_comments = true

icon_size = 42

row_radius = 12
row_margin = 2

search_background = "rgba(255, 255, 255, 0.07)"
search_border_color = "rgba(255, 255, 255, 0.08)"
search_focus_border_color = "rgba(130, 170, 255, 0.65)"
```

> Restart Raix after changing the configuration.

---

# ⌨️ Keyboard Controls

| Key         | Action              |
| ----------- | ------------------- |
| `↑`         | Move up             |
| `↓`         | Move down           |
| `Home`      | First application   |
| `End`       | Last application    |
| `Enter`     | Launch application  |
| `Esc`       | Close Raix          |
| `Backspace` | Delete search text  |
| Typing      | Search applications |

---

# 🐧 Hyprland

You can bind Raix to a keyboard shortcut in your Hyprland configuration.

For example:

```ini
bind = SUPER, R, exec, raix
```

Reload Hyprland:

```bash
hyprctl reload
```

Now press:

```text
Super + R
```

to open Raix.

---

# 🧪 Development

Clone the repository:

```bash
git clone https://github.com/raiyan323/raix.git
cd raix
```

Build in development mode:

```bash
cargo build
```

Run:

```bash
cargo run
```

For an optimized build:

```bash
cargo build --release
```

---

# 🏗️ Project Structure

```text
raix/
├── Cargo.toml
├── Cargo.lock
├── PKGBUILD
├── README.md
├── src/
│   └── main.rs
└── LICENSE
```

---

# 📜 License

Raix is open-source software licensed under the **MIT License**.

See [`LICENSE`](LICENSE) for the full license text.

---

# ⚡ Philosophy

Raix is built around a simple idea:

> **Your applications should be one command away.**

No mouse hunting.

No bloated desktop environment.

Just:

```text
Type → Find → Enter
```

Fast. Minimal. Keyboard-first.

---

## ⭐ Support

If you like Raix, consider giving the project a ⭐ on GitHub.

Issues, improvements, and pull requests are welcome.

---

**Raix — launch everything.**
