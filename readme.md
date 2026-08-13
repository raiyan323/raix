# Raix

> ⚡ A fast, minimal, hacker-style application launcher for Wayland.

**Raix** is a lightweight application launcher written in Rust, designed for Linux users who love the terminal, keyboard-driven workflows, and a cyberpunk aesthetic.

Built for **Wayland** and especially suited for compositors such as **Hyprland**.

---

## ✨ Features

- ⚡ Fast fuzzy application search
- 🧠 Keyboard-driven workflow
- 🐧 Linux / Wayland native
- 🎨 Fully configurable colors and layout
- 🖼️ Application icons
- 🔍 Fuzzy matching
- 🪶 Lightweight Rust implementation
- 🚀 Lazy icon loading
- 🔒 Single-instance protection
- 🖥️ Terminal application support
- 🎯 `wlr-layer-shell` integration

---

## 🖼️ Screenshot

> Add a screenshot here.

```text
┌────────────────────────────────────────────────────────────┐
│  > Search applications                                     │
│                                                            │
│  ▌   Alacritty                                            │
│      NetworkManager                                       │
│      Visual Studio Code                                   │
│      Foot                                                 │
│                                                            │
│  ↑ ↓ Navigate    Enter Launch    Esc Close                │
└────────────────────────────────────────────────────────────┘
```

---

# 📦 Arch Linux

## Requirements

Raix requires:

- Arch Linux
- Rust
- Cargo
- Wayland
- A Wayland compositor with `wlr-layer-shell` support
- `base-devel`

Install the required build tools:

```bash
sudo pacman -S --needed base-devel rust cargo wayland wayland-protocols
```

Depending on your system and dependencies, you may also need:

```bash
sudo pacman -S --needed pkgconf libxkbcommon
```

---

## 🚀 Build from Source

Clone the repository:

```bash
git clone https://github.com/YOUR_USERNAME/raix.git
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
width = 720
height = 520

anchor = "center"
margin_top = 80

padding = 20
row_height = 44
max_results = 12

corner_radius = 20
border_width = 1

show_icons = true
icon_size = 30
icon_gap = 14

font_path = ""

font_size = 17.0
prompt_font_size = 22.0

background = "#070b12"
opacity = 0.94

foreground = "#b8c7d9"
prompt_color = "#67e8f9"

selected_bg = "#102a3a"
selected_fg = "#e6fbff"

border_color = "#22d3ee"
search_background = "#0b1622"

show_hint = true

terminal = "foot"
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
