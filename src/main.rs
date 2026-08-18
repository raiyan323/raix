use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, CssProvider, Entry, EventControllerKey, Image,
    Label, ListBox, ListBoxRow, Orientation, ScrolledWindow,
};

use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use serde::{Deserialize, Serialize};

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;

// ============================================================
// APP INFO
// ============================================================

#[derive(Clone, Debug)]
struct AppInfo {
    name: String,
    exec: String,
    icon: String,
    comment: String,
    desktop_file: PathBuf,

    terminal: bool,
    dbus_activatable: bool,

    search_text: String,
}

// ============================================================
// CONFIG
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Config {
    width: i32,
    height: i32,

    background: String,
    text_color: String,
    comment_color: String,
    selected_color: String,
    hover_color: String,
    border_color: String,

    border_radius: i32,
    border_width: i32,

    font: String,

    app_font_size: i32,
    comment_font_size: i32,
    search_font_size: i32,

    search_placeholder: String,

    show_icons: bool,
    show_comments: bool,
    icon_size: i32,

    row_radius: i32,
    row_margin: i32,

    search_background: String,
    search_border_color: String,
    search_focus_border_color: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 760,
            height: 520,

            background: "rgba(8, 8, 10, 0.98)".to_string(),
            text_color: "#ffffff".to_string(),
            comment_color: "#888888".to_string(),

            selected_color: "rgba(255, 255, 255, 0.14)".to_string(),
            hover_color: "rgba(255, 255, 255, 0.07)".to_string(),
            border_color: "rgba(255, 255, 255, 0.16)".to_string(),

            border_radius: 18,
            border_width: 1,

            font: "JetBrainsMono Nerd Font".to_string(),

            app_font_size: 15,
            comment_font_size: 11,
            search_font_size: 17,

            search_placeholder: "Search applications...".to_string(),

            show_icons: true,
            show_comments: true,
            icon_size: 42,

            row_radius: 12,
            row_margin: 2,

            search_background: "rgba(255, 255, 255, 0.06)".to_string(),
            search_border_color: "rgba(255, 255, 255, 0.10)".to_string(),
            search_focus_border_color: "rgba(255, 255, 255, 0.55)".to_string(),
        }
    }
}

// ============================================================
// CONFIG PATH
// ============================================================

fn config_path() -> PathBuf {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    home.join(".config").join("raix").join("config.toml")
}

// ============================================================
// LOAD CONFIG
// ============================================================

fn load_config() -> Config {
    let path = config_path();

    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            eprintln!(
                "raix: cannot create config directory '{}': {}",
                parent.display(),
                error
            );
        }
    }

    if !path.exists() {
        let config = Config::default();

        match toml::to_string_pretty(&config) {
            Ok(data) => {
                if let Err(error) = std::fs::write(&path, data) {
                    eprintln!("raix: cannot create config '{}': {}", path.display(), error);
                }
            }

            Err(error) => {
                eprintln!("raix: cannot serialize default config: {}", error);
            }
        }

        return config;
    }

    match std::fs::read_to_string(&path) {
        Ok(data) => match toml::from_str::<Config>(&data) {
            Ok(config) => config,

            Err(error) => {
                eprintln!("raix: invalid config '{}': {}", path.display(), error);

                eprintln!("raix: using default configuration");

                Config::default()
            }
        },

        Err(error) => {
            eprintln!("raix: cannot read config '{}': {}", path.display(), error);

            Config::default()
        }
    }
}

// ============================================================
// MAIN
// ============================================================

fn main() {
    let app = Application::builder()
        .application_id("com.raiyan.raix")
        .build();

    app.connect_activate(build_ui);

    app.run();
}

// ============================================================
// BUILD UI
// ============================================================

fn build_ui(app: &Application) {
    let config = load_config();

    let applications = Rc::new(load_desktop_apps());

    eprintln!("raix: loaded {} application entries", applications.len());

    // ========================================================
    // WINDOW
    // ========================================================

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Raix")
        .decorated(false)
        .resizable(false)
        .build();

    // ========================================================
    // LAYER SHELL
    // ========================================================

    window.init_layer_shell();

    window.set_layer(Layer::Overlay);

    window.set_keyboard_mode(KeyboardMode::Exclusive);

    window.set_exclusive_zone(-1);

    window.set_namespace(Some("raix"));

    // Fixed-size centered layer-shell surface.
    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, false);
    window.set_anchor(Edge::Left, false);
    window.set_anchor(Edge::Right, false);

    window.set_margin(Edge::Top, 0);
    window.set_margin(Edge::Bottom, 0);
    window.set_margin(Edge::Left, 0);
    window.set_margin(Edge::Right, 0);

    // ========================================================
    // CSS
    // ========================================================

    let css = CssProvider::new();

    let css_data = format!(
        r#"
        * {{
            font-family:
                "{font}",
                "Noto Sans",
                sans-serif;
        }}

        window {{
            background: transparent;
        }}

        .launcher {{
            background: {background};

            border:
                {border_width}px solid
                {border_color};

            border-radius:
                {border_radius}px;

            padding: 14px;
        }}

        .search {{
            background:
                {search_background};

            color:
                {text_color};

            border:
                1px solid
                {search_border_color};

            border-radius:
                12px;

            padding:
                13px 16px;

            font-size:
                {search_font_size}px;

            min-height:
                28px;

            caret-color:
                #ffffff;
        }}

        .search:focus {{
            background:
                rgba(255, 255, 255, 0.09);

            border-color:
                {search_focus_border_color};
        }}

        .search placeholder {{
            color:
                #777777;
        }}

        /*
         * Only style the actual ListBox rows.
         *
         * Do NOT use a generic "row" selector globally.
         * GTK4 has many internal widgets and overly broad
         * CSS can interfere with ScrolledWindow internals.
         */

        list {{
            background: transparent;
        }}

        list > row {{
            background: transparent;

            border-radius:
                {row_radius}px;

            padding: 4px;

            margin:
                {row_margin}px 0;
        }}

        list > row:hover {{
            background:
                {hover_color};
        }}

        list > row:selected {{
            background:
                {selected_color};
        }}

        .app-name {{
            color:
                {text_color};

            font-size:
                {app_font_size}px;

            font-weight:
                600;
        }}

        .app-comment {{
            color:
                {comment_color};

            font-size:
                {comment_font_size}px;
        }}

        .icon {{
            min-width:
                {icon_size}px;

            min-height:
                {icon_size}px;
        }}
        "#,
        font = config.font,
        background = config.background,
        border_width = config.border_width,
        border_color = config.border_color,
        border_radius = config.border_radius,
        text_color = config.text_color,
        search_background = config.search_background,
        search_border_color = config.search_border_color,
        search_focus_border_color = config.search_focus_border_color,
        search_font_size = config.search_font_size,
        row_radius = config.row_radius,
        row_margin = config.row_margin,
        hover_color = config.hover_color,
        selected_color = config.selected_color,
        app_font_size = config.app_font_size,
        comment_color = config.comment_color,
        comment_font_size = config.comment_font_size,
        icon_size = config.icon_size,
    );

    css.load_from_data(&css_data);

    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    // ========================================================
    // MAIN CONTAINER
    // ========================================================

    let container = GtkBox::new(Orientation::Vertical, 10);

    container.set_width_request(config.width);
    container.set_height_request(config.height);

    container.add_css_class("launcher");

    // ========================================================
    // SEARCH
    // ========================================================

    let search = Entry::new();

    search.set_placeholder_text(Some(&config.search_placeholder));

    search.set_hexpand(true);

    search.add_css_class("search");

    container.append(&search);

    // ========================================================
    // LIST
    // ========================================================

    let list = ListBox::new();

    list.set_selection_mode(gtk4::SelectionMode::Single);

    list.set_vexpand(true);
    list.set_hexpand(true);

    list.set_activate_on_single_click(false);

    /*
     * GTK4 scrolling:
     *
     * - ScrolledWindow owns the viewport.
     * - ListBox owns rows.
     * - No custom scrollbar CSS.
     * - We use the native vertical adjustment only when
     *   keyboard navigation requires bringing a selected row
     *   into view.
     */

    let scroll = ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();

    container.append(&scroll);

    window.set_child(Some(&container));

    // ========================================================
    // RESULTS
    // ========================================================

    let initial_results: Vec<usize> = (0..applications.len()).collect();

    let current_results = Rc::new(RefCell::new(initial_results.clone()));

    populate_list(&list, &applications, &initial_results, &config);

    select_first_row(&list);

    // ========================================================
    // SEARCH
    // ========================================================

    {
        let list = list.clone();

        let applications = Rc::clone(&applications);

        let current_results = Rc::clone(&current_results);

        let config = config.clone();

        search.connect_changed(move |entry| {
            let query = entry.text().trim().to_lowercase();

            if query.is_empty() {
                let results: Vec<usize> = (0..applications.len()).collect();

                *current_results.borrow_mut() = results.clone();

                populate_list(&list, &applications, &results, &config);

                select_first_row(&list);

                return;
            }

            let mut scored: Vec<(i32, usize)> = Vec::with_capacity(applications.len());

            for (index, app) in applications.iter().enumerate() {
                if let Some(score) = fuzzy_score(&query, &app.search_text) {
                    scored.push((score, index));
                }
            }

            scored.sort_unstable_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| applications[a.1].name.cmp(&applications[b.1].name))
            });

            let results: Vec<usize> = scored.into_iter().map(|(_, index)| index).collect();

            *current_results.borrow_mut() = results.clone();

            populate_list(&list, &applications, &results, &config);

            select_first_row(&list);

            entry.grab_focus();

            // GTK4 Editable::set_position expects i32.
            entry.set_position(-1_i32);
        });
    }

    // ========================================================
    // ENTER
    // ========================================================

    {
        let list = list.clone();

        let applications = Rc::clone(&applications);

        let current_results = Rc::clone(&current_results);

        let window = window.clone();

        search.connect_activate(move |_| {
            launch_selected(&list, &applications, &current_results, &window);
        });
    }

    // ========================================================
    // KEYBOARD
    // ========================================================

    {
        let controller = EventControllerKey::new();

        let search_for_keys = search.clone();

        let list_for_keys = list.clone();

        let scroll_for_keys = scroll.clone();

        let window_for_keys = window.clone();

        let applications_for_keys = Rc::clone(&applications);

        let current_results_for_keys = Rc::clone(&current_results);

        controller.connect_key_pressed(move |_, key, _, _| {
            match key {
                // ------------------------------------------------
                // ESC
                // ------------------------------------------------
                gdk::Key::Escape => {
                    window_for_keys.hide();

                    gtk4::glib::Propagation::Stop
                }

                // ------------------------------------------------
                // DOWN
                // ------------------------------------------------
                gdk::Key::Down => {
                    move_selection(&list_for_keys, &scroll_for_keys, 1);

                    search_for_keys.grab_focus();

                    search_for_keys.set_position(-1_i32);

                    gtk4::glib::Propagation::Stop
                }

                // ------------------------------------------------
                // UP
                // ------------------------------------------------
                gdk::Key::Up => {
                    move_selection(&list_for_keys, &scroll_for_keys, -1);

                    search_for_keys.grab_focus();

                    search_for_keys.set_position(-1_i32);

                    gtk4::glib::Propagation::Stop
                }

                // ------------------------------------------------
                // PAGE DOWN
                // ------------------------------------------------
                gdk::Key::Page_Down => {
                    move_selection(&list_for_keys, &scroll_for_keys, 6);

                    search_for_keys.grab_focus();

                    search_for_keys.set_position(-1_i32);

                    gtk4::glib::Propagation::Stop
                }

                // ------------------------------------------------
                // PAGE UP
                // ------------------------------------------------
                gdk::Key::Page_Up => {
                    move_selection(&list_for_keys, &scroll_for_keys, -6);

                    search_for_keys.grab_focus();

                    search_for_keys.set_position(-1_i32);

                    gtk4::glib::Propagation::Stop
                }

                // ------------------------------------------------
                // ENTER
                // ------------------------------------------------
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    launch_selected(
                        &list_for_keys,
                        &applications_for_keys,
                        &current_results_for_keys,
                        &window_for_keys,
                    );

                    gtk4::glib::Propagation::Stop
                }

                _ => gtk4::glib::Propagation::Proceed,
            }
        });

        search.add_controller(controller);
    }

    // ========================================================
    // ROW ACTIVATION
    // ========================================================

    {
        let window = window.clone();

        let applications = Rc::clone(&applications);

        let current_results = Rc::clone(&current_results);

        list.connect_row_activated(move |_, row| {
            let position = row.index();

            if position < 0 {
                return;
            }

            let position = position as usize;

            let results = current_results.borrow();

            let Some(&app_index) = results.get(position) else {
                return;
            };

            let Some(app) = applications.get(app_index) else {
                return;
            };

            launch_app(app);

            window.hide();
        });
    }

    // ========================================================
    // KEEP SEARCH FOCUSED
    // ========================================================

    {
        let search = search.clone();

        list.connect_selected_rows_changed(move |_| {
            search.grab_focus();

            search.set_position(-1_i32);
        });
    }

    // ========================================================
    // CLOSE
    // ========================================================

    window.connect_close_request(|window| {
        window.hide();

        gtk4::glib::Propagation::Stop
    });

    // ========================================================
    // SHOW
    // ========================================================

    window.show();

    window.present();

    search.grab_focus();

    search.set_position(-1_i32);
}

// ============================================================
// SELECT FIRST
// ============================================================

fn select_first_row(list: &ListBox) {
    if let Some(row) = list.row_at_index(0) {
        list.select_row(Some(&row));
    }
}

// ============================================================
// LAUNCH SELECTED
// ============================================================

fn launch_selected(
    list: &ListBox,
    applications: &Rc<Vec<AppInfo>>,
    current_results: &Rc<RefCell<Vec<usize>>>,
    window: &ApplicationWindow,
) {
    let Some(row) = list.selected_row() else {
        return;
    };

    let position = row.index();

    if position < 0 {
        return;
    }

    let position = position as usize;

    let results = current_results.borrow();

    let Some(&app_index) = results.get(position) else {
        return;
    };

    let Some(app) = applications.get(app_index) else {
        return;
    };

    launch_app(app);

    window.hide();
}

// ============================================================
// MOVE SELECTION
// ============================================================

fn move_selection(list: &ListBox, scroll: &ScrolledWindow, amount: i32) {
    let count = list.observe_children().n_items() as i32;

    if count <= 0 {
        return;
    }

    let current = list.selected_row().map(|row| row.index()).unwrap_or(0);

    let next = current.saturating_add(amount).clamp(0, count - 1);

    let Some(row) = list.row_at_index(next) else {
        return;
    };

    list.select_row(Some(&row));

    // ========================================================
    // NATIVE GTK4 SCROLLED WINDOW ADJUSTMENT
    //
    // We do not style or manipulate GTK's scrollbar widget.
    // We only adjust the viewport when keyboard navigation
    // moves outside the visible area.
    // ========================================================

    scroll_selected_row_into_view(scroll, &row);
}

// ============================================================
// SCROLL SELECTED ROW INTO VIEW
// ============================================================

fn scroll_selected_row_into_view(scroll: &ScrolledWindow, row: &ListBoxRow) {
    let adjustment = scroll.vadjustment();

    let allocation = row.allocation();

    let row_top = allocation.y() as f64;

    let row_bottom = row_top + allocation.height() as f64;

    let viewport_top = adjustment.value();

    let viewport_bottom = viewport_top + adjustment.page_size();

    let mut target = viewport_top;

    if row_top < viewport_top {
        target = row_top;
    } else if row_bottom > viewport_bottom {
        target = row_bottom - adjustment.page_size();
    }

    let lower = adjustment.lower();

    let upper = adjustment.upper();

    let page_size = adjustment.page_size();

    let max_value = (upper - page_size).max(lower);

    target = target.clamp(lower, max_value);

    if (target - adjustment.value()).abs() > f64::EPSILON {
        adjustment.set_value(target);
    }
}

// ============================================================
// POPULATE LIST
// ============================================================

fn populate_list(
    list: &ListBox,
    applications: &Rc<Vec<AppInfo>>,
    indexes: &[usize],
    config: &Config,
) {
    while let Some(row) = list.row_at_index(0) {
        list.remove(&row);
    }

    for &index in indexes {
        let Some(app) = applications.get(index) else {
            continue;
        };

        let row = create_app_row(app, config);

        list.append(&row);
    }
}

// ============================================================
// CREATE APP ROW
// ============================================================

fn create_app_row(app: &AppInfo, config: &Config) -> ListBoxRow {
    let row = ListBoxRow::new();

    let wrapper = GtkBox::new(Orientation::Horizontal, 12);

    wrapper.set_margin_start(8);
    wrapper.set_margin_end(8);
    wrapper.set_margin_top(5);
    wrapper.set_margin_bottom(5);

    // ========================================================
    // ICON
    // ========================================================

    if config.show_icons {
        let image = if app.icon.contains('/') {
            Image::from_file(&app.icon)
        } else {
            Image::from_icon_name(&app.icon)
        };

        image.set_pixel_size(config.icon_size);

        image.add_css_class("icon");

        wrapper.append(&image);
    }

    // ========================================================
    // TEXT
    // ========================================================

    let text_box = GtkBox::new(Orientation::Vertical, 2);

    text_box.set_valign(gtk4::Align::Center);

    text_box.set_hexpand(true);

    // ========================================================
    // NAME
    // ========================================================

    let name = Label::new(Some(&app.name));

    name.set_halign(gtk4::Align::Start);

    name.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    name.set_hexpand(true);

    name.add_css_class("app-name");

    text_box.append(&name);

    // ========================================================
    // COMMENT
    // ========================================================

    if config.show_comments && !app.comment.is_empty() {
        let comment = Label::new(Some(&app.comment));

        comment.set_halign(gtk4::Align::Start);

        comment.set_ellipsize(gtk4::pango::EllipsizeMode::End);

        comment.set_hexpand(true);

        comment.add_css_class("app-comment");

        text_box.append(&comment);
    }

    wrapper.append(&text_box);

    row.set_child(Some(&wrapper));

    row
}

// ============================================================
// LAUNCH APPLICATION
// ============================================================

fn launch_app(app: &AppInfo) {
    // --------------------------------------------------------
    // DBusActivatable
    //
    // We intentionally do not use gio::DesktopAppInfo here.
    // gtk4's gio dependency does not expose that type in the
    // configuration currently being used.
    //
    // If DBus activation is present but unavailable through
    // the current API, falling back to Exec is safer than
    // preventing the application from launching.
    // --------------------------------------------------------

    if app.dbus_activatable {
        eprintln!(
            "raix: '{}' is DBusActivatable; using Exec fallback",
            app.name
        );
    }

    let Some(parts) = build_exec_arguments(app) else {
        eprintln!("raix: invalid Exec entry for '{}': {}", app.name, app.exec);

        return;
    };

    if parts.is_empty() {
        eprintln!("raix: empty Exec entry for '{}'", app.name);

        return;
    }

    let executable = &parts[0];

    let args = &parts[1..];

    // ========================================================
    // TERMINAL APP
    // ========================================================

    if app.terminal {
        if let Err(error) = launch_in_terminal(executable, args) {
            eprintln!(
                "raix: failed to launch terminal application '{}': {}",
                app.name, error
            );
        }

        return;
    }

    // ========================================================
    // NORMAL APP
    // ========================================================

    match Command::new(executable)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(_) => {}

        Err(error) => {
            eprintln!("raix: failed to launch '{}': {}", app.name, error);
        }
    }
}

// ============================================================
// TERMINAL
// ============================================================

fn launch_in_terminal(executable: &str, args: &[String]) -> Result<(), String> {
    let terminals = [
        "xdg-terminal-exec",
        "kitty",
        "foot",
        "wezterm",
        "alacritty",
        "ghostty",
        "konsole",
        "gnome-terminal",
        "xfce4-terminal",
        "xterm",
    ];

    for terminal in terminals {
        if !command_exists(terminal) {
            continue;
        }

        let mut command = Command::new(terminal);

        match terminal {
            "xdg-terminal-exec" => {
                command.arg("--").arg(executable).args(args);
            }

            "kitty" | "alacritty" | "ghostty" | "konsole" | "xterm" => {
                command.arg("-e").arg(executable).args(args);
            }

            "foot" => {
                command.arg(executable).args(args);
            }

            "wezterm" => {
                command.arg("start").arg("--").arg(executable).args(args);
            }

            "gnome-terminal" => {
                command.arg("--").arg(executable).args(args);
            }

            "xfce4-terminal" => {
                /*
                 * xfce4-terminal's --command accepts one
                 * shell-like command string. We therefore
                 * quote arguments before joining them.
                 */

                let command_line = std::iter::once(shell_quote(executable))
                    .chain(args.iter().map(|arg| shell_quote(arg)))
                    .collect::<Vec<_>>()
                    .join(" ");

                command.arg("--command").arg(command_line);
            }

            _ => continue,
        }

        match command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(_) => return Ok(()),

            Err(error) => {
                eprintln!("raix: terminal '{}' failed: {}", terminal, error);
            }
        }
    }

    Err("no supported terminal emulator found".to_string())
}

// ============================================================
// SHELL QUOTE
// ============================================================

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    if value.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '_' | '-' | '.' | '/' | ':' | '@' | '%' | '+' | '=')
    }) {
        return value.to_string();
    }

    let escaped = value.replace('\'', "'\\''");

    format!("'{}'", escaped)
}

// ============================================================
// COMMAND EXISTS
// ============================================================

fn command_exists(command: &str) -> bool {
    let path = Path::new(command);

    if path.components().count() > 1 {
        return path.is_file();
    }

    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    for directory in env::split_paths(&paths) {
        let candidate = directory.join(command);

        if candidate.is_file() {
            return true;
        }
    }

    false
}

// ============================================================
// BUILD EXEC ARGUMENTS
// ============================================================

fn build_exec_arguments(app: &AppInfo) -> Option<Vec<String>> {
    let tokens = desktop_exec_split(&app.exec)?;

    let mut result = Vec::with_capacity(tokens.len());

    for token in tokens {
        if token.is_empty() {
            continue;
        }

        match token.as_str() {
            "%f" | "%F" | "%u" | "%U" => {
                continue;
            }

            "%i" => {
                if !app.icon.is_empty() && app.icon != "application-x-executable" {
                    result.push("--icon".to_string());

                    result.push(app.icon.clone());
                }

                continue;
            }

            "%c" => {
                result.push(app.name.clone());

                continue;
            }

            "%k" => {
                result.push(app.desktop_file.to_string_lossy().into_owned());

                continue;
            }

            "%v" | "%m" => {
                continue;
            }

            _ => {}
        }

        if token.contains("%f")
            || token.contains("%F")
            || token.contains("%u")
            || token.contains("%U")
            || token.contains("%i")
            || token.contains("%v")
            || token.contains("%m")
        {
            continue;
        }

        let value = token
            .replace("%c", &app.name)
            .replace("%k", &app.desktop_file.to_string_lossy());

        result.push(value);
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

// ============================================================
// DESKTOP EXEC SPLITTER
// ============================================================

fn desktop_exec_split(input: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();

    let mut current = String::new();

    let mut single_quote = false;

    let mut double_quote = false;

    let mut escaped = false;

    let mut token_started = false;

    for c in input.chars() {
        if escaped {
            current.push(c);

            escaped = false;

            token_started = true;

            continue;
        }

        match c {
            '\\' if !single_quote => {
                escaped = true;

                token_started = true;
            }

            '\'' if !double_quote => {
                single_quote = !single_quote;

                token_started = true;
            }

            '"' if !single_quote => {
                double_quote = !double_quote;

                token_started = true;
            }

            ' ' | '\t' if !single_quote && !double_quote => {
                if token_started {
                    args.push(std::mem::take(&mut current));

                    token_started = false;
                }
            }

            _ => {
                current.push(c);

                token_started = true;
            }
        }
    }

    if escaped || single_quote || double_quote {
        return None;
    }

    if token_started {
        args.push(current);
    }

    Some(args)
}

// ============================================================
// LOAD DESKTOP APPS
// ============================================================

fn load_desktop_apps() -> Vec<AppInfo> {
    let directories = application_directories();

    let mut apps = Vec::new();

    let mut seen_files: HashSet<PathBuf> = HashSet::new();

    let mut seen_desktop_names: HashSet<String> = HashSet::new();

    let mut seen_apps: HashSet<String> = HashSet::new();

    for directory in directories {
        if !directory.is_dir() {
            continue;
        }

        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,

            Err(error) => {
                eprintln!("raix: cannot read '{}': {}", directory.display(), error);

                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();

            if path.extension().and_then(|x| x.to_str()) != Some("desktop") {
                continue;
            }

            // ====================================================
            // DESKTOP FILE NAME DEDUP
            // ====================================================

            if let Some(name) = path.file_name().and_then(|x| x.to_str()) {
                if !seen_desktop_names.insert(name.to_string()) {
                    continue;
                }
            }

            // ====================================================
            // CANONICAL PATH
            // ====================================================

            let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());

            if !seen_files.insert(canonical.clone()) {
                continue;
            }

            // ====================================================
            // READ
            // ====================================================

            let data = match std::fs::read_to_string(&path) {
                Ok(data) => data,

                Err(error) => {
                    eprintln!("raix: cannot read '{}': {}", path.display(), error);

                    continue;
                }
            };

            let Some(entry) = parse_desktop_entry(&data) else {
                continue;
            };

            // ====================================================
            // TYPE
            // ====================================================

            if entry
                .get("Type")
                .map(String::as_str)
                .unwrap_or("Application")
                != "Application"
            {
                continue;
            }

            // ====================================================
            // HIDDEN
            // ====================================================

            if parse_bool(entry.get("Hidden")) || parse_bool(entry.get("NoDisplay")) {
                continue;
            }

            // ====================================================
            // TRY EXEC
            // ====================================================

            if let Some(try_exec) = entry.get("TryExec") {
                let try_exec = try_exec.trim();

                if !try_exec.is_empty() && !command_exists(try_exec) {
                    continue;
                }
            }

            // ====================================================
            // DESKTOP ENVIRONMENT
            // ====================================================

            if !desktop_environment_allows(&entry) {
                continue;
            }

            // ====================================================
            // NAME
            // ====================================================

            let name = localized_value(&entry, "Name").unwrap_or_default();

            if name.trim().is_empty() {
                continue;
            }

            // ====================================================
            // EXEC
            // ====================================================

            let exec = entry.get("Exec").cloned().unwrap_or_default();

            if exec.trim().is_empty() {
                continue;
            }

            if desktop_exec_split(&exec).is_none() {
                eprintln!("raix: malformed Exec in '{}'", path.display());

                continue;
            }

            // ====================================================
            // ICON
            // ====================================================

            let icon = entry
                .get("Icon")
                .cloned()
                .filter(|x| !x.trim().is_empty())
                .unwrap_or_else(|| "application-x-executable".to_string());

            // ====================================================
            // COMMENT
            // ====================================================

            let comment = localized_value(&entry, "Comment").unwrap_or_default();

            // ====================================================
            // TERMINAL
            // ====================================================

            let terminal = parse_bool(entry.get("Terminal"));

            // ====================================================
            // DBUS
            // ====================================================

            let dbus_activatable = parse_bool(entry.get("DBusActivatable"));

            // ====================================================
            // DUPLICATE APP
            // ====================================================

            let duplicate_key = format!("{}\n{}", name.to_lowercase(), exec.to_lowercase());

            if !seen_apps.insert(duplicate_key) {
                continue;
            }

            // ====================================================
            // SEARCH TEXT
            // ====================================================

            let search_text = format!(
                "{} {} {}",
                name.to_lowercase(),
                comment.to_lowercase(),
                exec.to_lowercase()
            );

            apps.push(AppInfo {
                name,
                exec,
                icon,
                comment,
                desktop_file: canonical,
                terminal,
                dbus_activatable,
                search_text,
            });
        }
    }

    // ========================================================
    // SORT
    // ========================================================

    apps.sort_unstable_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });

    apps
}

// ============================================================
// APPLICATION DIRECTORIES
// ============================================================

fn application_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();

    // ========================================================
    // USER
    // ========================================================

    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        directories.push(PathBuf::from(data_home).join("applications"));
    } else if let Some(home) = env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/share/applications"));
    }

    // ========================================================
    // SYSTEM
    // ========================================================

    if let Some(data_dirs) = env::var_os("XDG_DATA_DIRS") {
        for directory in env::split_paths(&data_dirs) {
            directories.push(directory.join("applications"));
        }
    }

    directories.push(PathBuf::from("/usr/local/share/applications"));

    directories.push(PathBuf::from("/usr/share/applications"));

    // ========================================================
    // DEDUP
    // ========================================================

    let mut result = Vec::new();

    let mut seen: HashSet<PathBuf> = HashSet::new();

    for directory in directories {
        if seen.insert(directory.clone()) {
            result.push(directory);
        }
    }

    result
}

// ============================================================
// DESKTOP ENTRY PARSER
// ============================================================

fn parse_desktop_entry(data: &str) -> Option<HashMap<String, String>> {
    let mut map: HashMap<String, String> = HashMap::new();

    let mut in_entry = false;

    for raw_line in data.lines() {
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if line == "[Desktop Entry]" {
            in_entry = true;

            continue;
        }

        if line.starts_with('[') {
            if in_entry {
                break;
            }

            continue;
        }

        if !in_entry {
            continue;
        }

        let Some((key, value)) = split_key_value(line) else {
            continue;
        };

        map.insert(key.to_string(), unescape_desktop_value(value));
    }

    if map.is_empty() { None } else { Some(map) }
}

// ============================================================
// KEY VALUE
// ============================================================

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let position = line.find('=')?;

    let key = line[..position].trim();

    if key.is_empty() {
        return None;
    }

    Some((key, &line[position + 1..]))
}

// ============================================================
// UNESCAPE
// ============================================================

fn unescape_desktop_value(value: &str) -> String {
    let mut result = String::with_capacity(value.len());

    let mut chars = value.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);

            continue;
        }

        match chars.next() {
            Some('s') => result.push(' '),

            Some('n') => result.push('\n'),

            Some('t') => result.push('\t'),

            Some('r') => result.push('\r'),

            Some('\\') => result.push('\\'),

            Some(other) => {
                result.push('\\');

                result.push(other);
            }

            None => {
                result.push('\\');
            }
        }
    }

    result
}

// ============================================================
// LOCALIZED VALUE
// ============================================================

fn localized_value(entry: &HashMap<String, String>, base: &str) -> Option<String> {
    let locale = current_locale();

    let mut candidates: Vec<String> = Vec::new();

    if !locale.is_empty() {
        candidates.push(locale.clone());

        if let Some(index) = locale.find('@') {
            candidates.push(locale[..index].to_string());
        }

        if let Some(index) = locale.find('.') {
            candidates.push(locale[..index].to_string());
        }

        if let Some(index) = locale.find('_') {
            candidates.push(locale[..index].to_string());
        }
    }

    let mut seen: HashSet<String> = HashSet::new();

    for locale in candidates {
        if !seen.insert(locale.clone()) {
            continue;
        }

        let key = format!("{}[{}]", base, locale);

        if let Some(value) = entry.get(&key) {
            if !value.is_empty() {
                return Some(value.clone());
            }
        }
    }

    entry.get(base).cloned()
}

// ============================================================
// LOCALE
// ============================================================

fn current_locale() -> String {
    for variable in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = env::var(variable) {
            if !value.is_empty() && value != "C" && value != "POSIX" {
                return value;
            }
        }
    }

    String::new()
}

// ============================================================
// BOOLEAN
// ============================================================

fn parse_bool(value: Option<&String>) -> bool {
    value
        .map(|x| x.trim())
        .is_some_and(|x| x.eq_ignore_ascii_case("true"))
}

// ============================================================
// DESKTOP FILTER
// ============================================================

fn desktop_environment_allows(entry: &HashMap<String, String>) -> bool {
    let current = current_desktop_names();

    if let Some(value) = entry.get("OnlyShowIn") {
        let allowed = split_semicolon_list(value);

        if !allowed.is_empty()
            && !allowed.iter().any(|desktop| {
                current
                    .iter()
                    .any(|current_desktop| current_desktop.eq_ignore_ascii_case(desktop))
            })
        {
            return false;
        }
    }

    if let Some(value) = entry.get("NotShowIn") {
        let blocked = split_semicolon_list(value);

        if blocked.iter().any(|desktop| {
            current
                .iter()
                .any(|current_desktop| current_desktop.eq_ignore_ascii_case(desktop))
        }) {
            return false;
        }
    }

    true
}

// ============================================================
// CURRENT DESKTOP
// ============================================================

fn current_desktop_names() -> Vec<String> {
    let mut desktops: Vec<String> = Vec::new();

    // --------------------------------------------------------
    // XDG_CURRENT_DESKTOP
    // --------------------------------------------------------

    if let Ok(value) = env::var("XDG_CURRENT_DESKTOP") {
        for item in value.split(':') {
            let item = item.trim();

            if item.is_empty() {
                continue;
            }

            if !desktops
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(item))
            {
                desktops.push(item.to_string());
            }
        }
    }

    // --------------------------------------------------------
    // XDG_SESSION_DESKTOP
    // --------------------------------------------------------

    if let Ok(value) = env::var("XDG_SESSION_DESKTOP") {
        let value = value.trim();

        if !value.is_empty()
            && !desktops
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(value))
        {
            desktops.push(value.to_string());
        }
    }

    // --------------------------------------------------------
    // Hyprland fallback
    // --------------------------------------------------------

    if desktops.is_empty() && env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        desktops.push("Hyprland".to_string());
    }

    desktops
}

// ============================================================
// SEMICOLON LIST
// ============================================================

fn split_semicolon_list(value: &str) -> Vec<String> {
    value
        .split(';')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(ToString::to_string)
        .collect()
}

// ============================================================
// FUZZY SEARCH
// ============================================================

fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    // --------------------------------------------------------
    // Exact substring
    // --------------------------------------------------------

    if let Some(position) = text.find(query) {
        let position = i32::try_from(position).unwrap_or(i32::MAX);

        return Some(10_000_i32.saturating_sub(position.saturating_mul(2)));
    }

    // --------------------------------------------------------
    // Character fuzzy match
    // --------------------------------------------------------

    let mut query_iter = query.chars();

    let mut current = query_iter.next();

    let mut score: i32 = 0;

    let mut consecutive: i32 = 0;

    let mut previous: Option<char> = None;

    for (index, ch) in text.chars().enumerate() {
        let Some(target) = current else {
            break;
        };

        if ch == target {
            score = score.saturating_add(50);

            if consecutive > 0 {
                score = score.saturating_add(30);
            }

            if index == 0 {
                score = score.saturating_add(100);
            }

            if let Some(previous_char) = previous {
                if is_word_boundary(previous_char) {
                    score = score.saturating_add(80);
                }
            }

            consecutive = consecutive.saturating_add(1);

            current = query_iter.next();
        } else {
            consecutive = 0;
        }

        previous = Some(ch);
    }

    if current.is_none() { Some(score) } else { None }
}

// ============================================================
// WORD BOUNDARY
// ============================================================

fn is_word_boundary(c: char) -> bool {
    matches!(c, ' ' | '-' | '_' | '/' | '.' | ':' | '\\')
}
