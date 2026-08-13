use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{
    Application,
    ApplicationWindow,
    Box as GtkBox,
    CssProvider,
    Entry,
    EventControllerKey,
    Image,
    Label,
    ListBox,
    ListBoxRow,
    Orientation,
    ScrolledWindow,
};

use gtk4_layer_shell::{
    KeyboardMode,
    Layer,
    LayerShell,
};

use serde::{Deserialize, Serialize};

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
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
}

// ============================================================
// CONFIG
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Config {
    // Window
    width: i32,
    height: i32,

    // Appearance
    background: String,
    text_color: String,
    comment_color: String,
    selected_color: String,
    hover_color: String,
    border_color: String,

    border_radius: i32,
    border_width: i32,

    // Font
    font: String,
    app_font_size: i32,
    comment_font_size: i32,
    search_font_size: i32,

    // Search
    search_placeholder: String,

    // Apps
    show_icons: bool,
    show_comments: bool,
    icon_size: i32,

    // List
    row_radius: i32,
    row_margin: i32,

    // Search appearance
    search_background: String,
    search_border_color: String,
    search_focus_border_color: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 760,
            height: 520,

            background: "rgba(20, 20, 28, 0.97)".to_string(),

            text_color: "#eeeeee".to_string(),

            comment_color: "#858591".to_string(),

            selected_color: "rgba(110, 140, 255, 0.22)".to_string(),

            hover_color: "rgba(255, 255, 255, 0.07)".to_string(),

            border_color: "rgba(255, 255, 255, 0.10)".to_string(),

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

            search_background: "rgba(255, 255, 255, 0.07)".to_string(),

            search_border_color: "rgba(255, 255, 255, 0.08)".to_string(),

            search_focus_border_color:
                "rgba(130, 170, 255, 0.65)".to_string(),
        }
    }
}

// ============================================================
// CONFIG PATH
// ============================================================

fn config_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    home.join(".config")
        .join("raix")
        .join("config.toml")
}

// ============================================================
// LOAD CONFIG
// ============================================================

fn load_config() -> Config {
    let path = config_path();

    if let Some(parent) = path.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            eprintln!(
                "raix: cannot create config directory: {}",
                error
            );
        }
    }

    if !path.exists() {
        let config = Config::default();

        match toml::to_string_pretty(&config) {
            Ok(data) => {
                if let Err(error) = std::fs::write(&path, data) {
                    eprintln!(
                        "raix: cannot create config: {}",
                        error
                    );
                }
            }

            Err(error) => {
                eprintln!(
                    "raix: cannot serialize config: {}",
                    error
                );
            }
        }

        return config;
    }

    match std::fs::read_to_string(&path) {
        Ok(data) => {
            match toml::from_str::<Config>(&data) {
                Ok(config) => config,

                Err(error) => {
                    eprintln!(
                        "raix: config error: {}",
                        error
                    );

                    Config::default()
                }
            }
        }

        Err(error) => {
            eprintln!(
                "raix: cannot read config: {}",
                error
            );

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
// SHOW LAUNCHER
// ============================================================

fn show_launcher(
    window: &ApplicationWindow,
) {
    window.show();
    window.present();
}

// ============================================================
// BUILD UI
// ============================================================

fn build_ui(app: &Application) {
    // ========================================================
    // IMPORTANT:
    //
    // GTK may call activate again.
    //
    // Never create another window if one already exists.
    // ========================================================

    if let Some(existing) = app.windows().first() {
        if let Some(window) =
            existing.downcast_ref::<ApplicationWindow>()
        {
            show_launcher(window);
        }

        return;
    }

    let config = load_config();

    let applications = Rc::new(load_desktop_apps());

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

    window.set_keyboard_mode(
        KeyboardMode::Exclusive,
    );

    window.set_exclusive_zone(-1);

    window.set_namespace(
        Some("raix"),
    );

    // Center the surface.
    window.set_anchor(
        gtk4_layer_shell::Edge::Top,
        false,
    );

    window.set_anchor(
        gtk4_layer_shell::Edge::Bottom,
        false,
    );

    window.set_anchor(
        gtk4_layer_shell::Edge::Left,
        false,
    );

    window.set_anchor(
        gtk4_layer_shell::Edge::Right,
        false,
    );

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
            background:
                transparent;
        }}

        .launcher {{
            background:
                {background};

            border:
                {border_width}px solid
                {border_color};

            border-radius:
                {border_radius}px;

            padding:
                14px;
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
            border-color:
                {search_focus_border_color};

            background:
                rgba(
                    255,
                    255,
                    255,
                    0.09
                );
        }}

        .search placeholder {{
            color:
                #777783;
        }}

        list {{
            background:
                transparent;
        }}

        row {{
            background:
                transparent;

            border-radius:
                {row_radius}px;

            padding:
                4px;

            margin:
                {row_margin}px 0;
        }}

        row:hover {{
            background:
                {hover_color};
        }}

        row:selected {{
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

        scrollbar {{
            background:
                transparent;
        }}

        scrollbar slider {{
            background:
                rgba(
                    255,
                    255,
                    255,
                    0.15
                );

            border-radius:
                8px;
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
        search_focus_border_color =
            config.search_focus_border_color,
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

    let container = GtkBox::new(
        Orientation::Vertical,
        10,
    );

    container.set_width_request(
        config.width,
    );

    container.set_height_request(
        config.height,
    );

    container.add_css_class(
        "launcher",
    );

    // ========================================================
    // SEARCH
    // ========================================================

    let search = Entry::new();

    search.set_placeholder_text(
        Some(
            &config.search_placeholder,
        ),
    );

    search.set_hexpand(true);

    search.set_vexpand(false);

    search.add_css_class(
        "search",
    );

    container.append(
        &search,
    );

    // ========================================================
    // LIST
    // ========================================================

    let list = ListBox::new();

    list.set_selection_mode(
        gtk4::SelectionMode::Single,
    );

    list.set_vexpand(true);

    list.set_hexpand(true);

    list.set_activate_on_single_click(
        false,
    );

    let scroll = ScrolledWindow::builder()
        .child(&list)
        .vexpand(true)
        .hexpand(true)
        .hscrollbar_policy(
            gtk4::PolicyType::Never,
        )
        .vscrollbar_policy(
            gtk4::PolicyType::Automatic,
        )
        .build();

    container.append(
        &scroll,
    );

    window.set_child(
        Some(&container),
    );

    // ========================================================
    // CURRENT APPS
    // ========================================================

    let current_apps =
        Rc::new(
            RefCell::new(
                applications
                    .as_ref()
                    .clone(),
            ),
        );

    // ========================================================
    // INITIAL LIST
    // ========================================================

    populate_list(
        &list,
        &applications,
        &config,
    );

    if let Some(row) =
        list.row_at_index(0)
    {
        list.select_row(
            Some(&row),
        );
    }

    // ========================================================
    // SEARCH FILTER
    // ========================================================

    {
        let list = list.clone();

        let applications =
            applications.clone();

        let current_apps =
            current_apps.clone();

        let config =
            config.clone();

        search.connect_changed(
            move |entry| {
                let query =
                    entry
                        .text()
                        .to_lowercase();

                let filtered =
                    if query.is_empty() {
                        applications
                            .as_ref()
                            .clone()
                    } else {
                        let mut scored:
                            Vec<(i32, AppInfo)> =
                            applications
                                .iter()
                                .filter_map(
                                    |app| {
                                        let text =
                                            format!(
                                                "{} {} {}",
                                                app.name
                                                    .to_lowercase(),
                                                app.comment
                                                    .to_lowercase(),
                                                app.exec
                                                    .to_lowercase()
                                            );

                                        fuzzy_score(
                                            &query,
                                            &text,
                                        )
                                        .map(
                                            |score| {
                                                (
                                                    score,
                                                    app.clone(),
                                                )
                                            },
                                        )
                                    },
                                )
                                .collect();

                        scored.sort_by(
                            |a, b| {
                                b.0.cmp(&a.0)
                            },
                        );

                        scored
                            .into_iter()
                            .map(
                                |(_, app)| app,
                            )
                            .collect()
                    };

                *current_apps
                    .borrow_mut() =
                    filtered.clone();

                populate_list(
                    &list,
                    &Rc::new(filtered),
                    &config,
                );

                if let Some(row) =
                    list.row_at_index(0)
                {
                    list.select_row(
                        Some(&row),
                    );
                }

                entry.grab_focus();

                entry.set_position(-1);
            },
        );
    }

    // ========================================================
    // ENTER
    // ========================================================

    {
        let list_for_enter =
            list.clone();

        let current_apps_for_enter =
            current_apps.clone();

        let window_for_enter =
            window.clone();

        search.connect_activate(
            move |_| {
                launch_selected(
                    &list_for_enter,
                    &current_apps_for_enter,
                    &window_for_enter,
                );
            },
        );
    }

    // ========================================================
    // KEYBOARD
    // ========================================================

    {
        let controller =
            EventControllerKey::new();

        // IMPORTANT:
        // Clone search before moving it into closure.
        let search_for_keys =
            search.clone();

        let list_for_keys =
            list.clone();

        let scroll_for_keys =
            scroll.clone();

        let window_for_keys =
            window.clone();

        controller.connect_key_pressed(
            move |_, key, _, _| {
                match key {
                    // =========================================
                    // ESC
                    // =========================================

                    gdk::Key::Escape => {
                        window_for_keys.hide();

                        gtk4::glib::Propagation::Stop
                    }

                    // =========================================
                    // DOWN
                    // =========================================

                    gdk::Key::Down => {
                        move_selection(
                            &list_for_keys,
                            &scroll_for_keys,
                            1,
                        );

                        search_for_keys
                            .grab_focus();

                        search_for_keys
                            .set_position(-1);

                        gtk4::glib::Propagation::Stop
                    }

                    // =========================================
                    // UP
                    // =========================================

                    gdk::Key::Up => {
                        move_selection(
                            &list_for_keys,
                            &scroll_for_keys,
                            -1,
                        );

                        search_for_keys
                            .grab_focus();

                        search_for_keys
                            .set_position(-1);

                        gtk4::glib::Propagation::Stop
                    }

                    // =========================================
                    // PAGE DOWN
                    // =========================================

                    gdk::Key::Page_Down => {
                        move_selection(
                            &list_for_keys,
                            &scroll_for_keys,
                            6,
                        );

                        search_for_keys
                            .grab_focus();

                        search_for_keys
                            .set_position(-1);

                        gtk4::glib::Propagation::Stop
                    }

                    // =========================================
                    // PAGE UP
                    // =========================================

                    gdk::Key::Page_Up => {
                        move_selection(
                            &list_for_keys,
                            &scroll_for_keys,
                            -6,
                        );

                        search_for_keys
                            .grab_focus();

                        search_for_keys
                            .set_position(-1);

                        gtk4::glib::Propagation::Stop
                    }

                    // =========================================
                    // EVERYTHING ELSE
                    // =========================================

                    _ => {
                        gtk4::glib::Propagation::Proceed
                    }
                }
            },
        );

        // search is still available here because the closure
        // owns search_for_keys, not search itself.
        search.add_controller(
            controller,
        );
    }

    // ========================================================
    // ROW ACTIVATION
    // ========================================================

    {
        let window_for_row =
            window.clone();

        let current_apps_for_row =
            current_apps.clone();

        list.connect_row_activated(
            move |_, row| {
                let index =
                    row.index();

                if index < 0 {
                    return;
                }

                let apps =
                    current_apps_for_row
                        .borrow();

                if let Some(app) =
                    apps.get(
                        index as usize,
                    )
                {
                    launch_app(app);

                    window_for_row.hide();
                }
            },
        );
    }

    // ========================================================
    // KEEP SEARCH FOCUSED
    // ========================================================

    {
        let search_focus =
            search.clone();

        list.connect_selected_rows_changed(
            move |_| {
                search_focus
                    .grab_focus();

                search_focus
                    .set_position(-1);
            },
        );
    }

    // ========================================================
    // CLOSE = HIDE
    // ========================================================

    {
        window.connect_close_request(
            |window| {
                window.hide();

                gtk4::glib::Propagation::Stop
            },
        );
    }

    // ========================================================
    // PRESENT
    // ========================================================

    window.present();

    search.grab_focus();

    search.set_position(-1);
}

// ============================================================
// LAUNCH SELECTED
// ============================================================

fn launch_selected(
    list: &ListBox,
    current_apps: &Rc<RefCell<Vec<AppInfo>>>,
    window: &ApplicationWindow,
) {
    let Some(row) =
        list.selected_row()
    else {
        return;
    };

    let index =
        row.index();

    if index < 0 {
        return;
    }

    let apps =
        current_apps.borrow();

    if let Some(app) =
        apps.get(index as usize)
    {
        launch_app(app);

        window.hide();
    }
}

// ============================================================
// MOVE SELECTION
// ============================================================

fn move_selection(
    list: &ListBox,
    scroll: &ScrolledWindow,
    amount: i32,
) {
    let count =
        list.observe_children()
            .n_items() as i32;

    if count <= 0 {
        return;
    }

    let current =
        list.selected_row()
            .map(|row| row.index())
            .unwrap_or(0);

    let mut next =
        current + amount;

    if next < 0 {
        next = 0;
    }

    if next >= count {
        next = count - 1;
    }

    let Some(row) =
        list.row_at_index(next)
    else {
        return;
    };

    list.select_row(
        Some(&row),
    );

    // ========================================================
    // AUTO SCROLL
    // ========================================================

    let adjustment =
        scroll.vadjustment();

    let allocation =
        row.allocation();

    let row_top =
        allocation.y() as f64;

    let row_bottom =
        row_top
            + allocation.height() as f64;

    let current_value =
        adjustment.value();

    let page_size =
        adjustment.page_size();

    let viewport_top =
        current_value;

    let viewport_bottom =
        current_value + page_size;

    let mut new_value =
        current_value;

    if row_top < viewport_top {
        new_value = row_top;
    } else if row_bottom > viewport_bottom {
        new_value =
            row_bottom - page_size;
    }

    let lower =
        adjustment.lower();

    let upper =
        adjustment.upper();

    let max_value =
        (upper - page_size).max(lower);

    new_value =
        new_value
            .max(lower)
            .min(max_value);

    if (new_value - current_value).abs()
        > f64::EPSILON
    {
        adjustment.set_value(
            new_value,
        );
    }
}

// ============================================================
// POPULATE LIST
// ============================================================

fn populate_list(
    list: &ListBox,
    apps: &Rc<Vec<AppInfo>>,
    config: &Config,
) {
    while let Some(row) =
        list.row_at_index(0)
    {
        list.remove(&row);
    }

    for app in apps.iter() {
        let row =
            create_app_row(
                app,
                config,
            );

        list.append(&row);
    }
}

// ============================================================
// CREATE APP ROW
// ============================================================

fn create_app_row(
    app: &AppInfo,
    config: &Config,
) -> ListBoxRow {
    let row =
        ListBoxRow::new();

    let wrapper =
        GtkBox::new(
            Orientation::Horizontal,
            12,
        );

    wrapper.set_margin_start(8);
    wrapper.set_margin_end(8);
    wrapper.set_margin_top(5);
    wrapper.set_margin_bottom(5);

    // ========================================================
    // ICON
    // ========================================================

    if config.show_icons {
        let image =
            Image::from_icon_name(
                &app.icon,
            );

        image.set_pixel_size(
            config.icon_size,
        );

        image.add_css_class(
            "icon",
        );

        wrapper.append(
            &image,
        );
    }

    // ========================================================
    // TEXT
    // ========================================================

    let text_box =
        GtkBox::new(
            Orientation::Vertical,
            2,
        );

    text_box.set_valign(
        gtk4::Align::Center,
    );

    text_box.set_hexpand(true);

    // ========================================================
    // NAME
    // ========================================================

    let name =
        Label::new(
            Some(&app.name),
        );

    name.set_halign(
        gtk4::Align::Start,
    );

    name.set_ellipsize(
        gtk4::pango::EllipsizeMode::End,
    );

    name.add_css_class(
        "app-name",
    );

    text_box.append(
        &name,
    );

    // ========================================================
    // COMMENT
    // ========================================================

    if config.show_comments
        && !app.comment.is_empty()
    {
        let comment =
            Label::new(
                Some(&app.comment),
            );

        comment.set_halign(
            gtk4::Align::Start,
        );

        comment.set_ellipsize(
            gtk4::pango::EllipsizeMode::End,
        );

        comment.add_css_class(
            "app-comment",
        );

        text_box.append(
            &comment,
        );
    }

    wrapper.append(
        &text_box,
    );

    row.set_child(
        Some(&wrapper),
    );

    row
}

// ============================================================
// LAUNCH APPLICATION
// ============================================================

fn launch_app(
    app: &AppInfo,
) {
    let command =
        clean_exec(
            &app.exec,
        );

    if command.is_empty() {
        return;
    }

    let mut parts =
        shell_split(
            &command,
        );

    if parts.is_empty() {
        return;
    }

    let executable =
        parts.remove(0);

    match Command::new(
        executable,
    )
    .args(parts)
    .spawn()
    {
        Ok(_) => {}

        Err(error) => {
            eprintln!(
                "raix: failed to launch '{}': {}",
                app.name,
                error
            );
        }
    }
}

// ============================================================
// CLEAN EXEC
// ============================================================

fn clean_exec(
    exec: &str,
) -> String {
    let field_codes = [
        "%f",
        "%F",
        "%u",
        "%U",
        "%i",
        "%c",
        "%k",
        "%v",
        "%m",
    ];

    let mut result =
        exec.to_string();

    for code in field_codes {
        result =
            result.replace(
                code,
                "",
            );
    }

    result.trim().to_string()
}

// ============================================================
// SHELL SPLIT
// ============================================================

fn shell_split(
    input: &str,
) -> Vec<String> {
    let mut args =
        Vec::new();

    let mut current =
        String::new();

    let mut single_quote =
        false;

    let mut double_quote =
        false;

    let mut escaped =
        false;

    for c in input.chars() {
        if escaped {
            current.push(c);

            escaped = false;

            continue;
        }

        match c {
            '\\' if !single_quote => {
                escaped = true;
            }

            '\'' if !double_quote => {
                single_quote =
                    !single_quote;
            }

            '"' if !single_quote => {
                double_quote =
                    !double_quote;
            }

            ' ' | '\t'
                if !single_quote
                    && !double_quote =>
            {
                if !current.is_empty() {
                    args.push(
                        std::mem::take(
                            &mut current,
                        ),
                    );
                }
            }

            _ => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        args.push(current);
    }

    args
}

// ============================================================
// LOAD DESKTOP APPLICATIONS
// ============================================================

fn load_desktop_apps() -> Vec<AppInfo> {
    let mut directories =
        Vec::new();

    directories.push(
        PathBuf::from(
            "/usr/share/applications",
        ),
    );

    directories.push(
        PathBuf::from(
            "/usr/local/share/applications",
        ),
    );

    if let Some(home) =
        std::env::var_os("HOME")
    {
        directories.push(
            PathBuf::from(home)
                .join(
                    ".local/share/applications",
                ),
        );
    }

    let mut apps =
        Vec::new();

    let mut seen =
        HashSet::new();

    for directory in directories {
        if !directory.exists() {
            continue;
        }

        let entries =
            match std::fs::read_dir(
                &directory,
            ) {
                Ok(entries) => entries,

                Err(error) => {
                    eprintln!(
                        "raix: cannot read {}: {}",
                        directory.display(),
                        error
                    );

                    continue;
                }
            };

        for entry in entries.flatten() {
            let path =
                entry.path();

            if path
                .extension()
                .and_then(|x| x.to_str())
                != Some("desktop")
            {
                continue;
            }

            let data =
                match std::fs::read_to_string(
                    &path,
                ) {
                    Ok(data) => data,

                    Err(_) => continue,
                };

            let mut name =
                String::new();

            let mut exec =
                String::new();

            let mut icon =
                "application-x-executable"
                    .to_string();

            let mut comment =
                String::new();

            let mut in_desktop_entry =
                false;

            let mut hidden =
                false;

            let mut no_display =
                false;

            for line in data.lines() {
                let line =
                    line.trim();

                if line ==
                    "[Desktop Entry]"
                {
                    in_desktop_entry =
                        true;

                    continue;
                }

                if line.starts_with('[')
                    && line
                        != "[Desktop Entry]"
                {
                    in_desktop_entry =
                        false;

                    continue;
                }

                if !in_desktop_entry {
                    continue;
                }

                if let Some(value) =
                    line.strip_prefix("Name=")
                {
                    if name.is_empty() {
                        name =
                            value.to_string();
                    }
                }

                if let Some(value) =
                    line.strip_prefix("Exec=")
                {
                    exec =
                        value.to_string();
                }

                if let Some(value) =
                    line.strip_prefix("Icon=")
                {
                    icon =
                        value.to_string();
                }

                if let Some(value) =
                    line.strip_prefix("Comment=")
                {
                    comment =
                        value.to_string();
                }

                if let Some(value) =
                    line.strip_prefix("Hidden=")
                {
                    hidden =
                        value.eq_ignore_ascii_case(
                            "true",
                        );
                }

                if let Some(value) =
                    line.strip_prefix("NoDisplay=")
                {
                    no_display =
                        value.eq_ignore_ascii_case(
                            "true",
                        );
                }
            }

            if hidden || no_display {
                continue;
            }

            if name.is_empty()
                || exec.is_empty()
            {
                continue;
            }

            let key =
                format!(
                    "{}:{}",
                    name,
                    exec
                );

            if !seen.insert(key) {
                continue;
            }

            apps.push(
                AppInfo {
                    name,
                    exec,
                    icon,
                    comment,
                },
            );
        }
    }

    apps.sort_by_key(
        |app| {
            app.name.to_lowercase()
        },
    );

    apps
}

// ============================================================
// FUZZY SEARCH
// ============================================================

fn fuzzy_score(
    query: &str,
    text: &str,
) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    // Exact substring.
    if let Some(position) =
        text.find(query)
    {
        return Some(
            10000
                - position as i32,
        );
    }

    let query_chars =
        query
            .chars()
            .collect::<Vec<_>>();

    if query_chars.is_empty() {
        return Some(0);
    }

    let text_chars =
        text
            .chars()
            .collect::<Vec<_>>();

    let mut query_index =
        0usize;

    let mut score =
        0i32;

    let mut consecutive =
        0i32;

    for (index, ch) in
        text_chars.iter().enumerate()
    {
        if query_index >=
            query_chars.len()
        {
            break;
        }

        if *ch ==
            query_chars[query_index]
        {
            score += 50;

            if consecutive > 0 {
                score += 30;
            }

            if index == 0 {
                score += 100;
            }

            if index > 0 {
                let previous =
                    text_chars[index - 1];

                if previous == ' '
                    || previous == '-'
                    || previous == '_'
                    || previous == '/'
                    || previous == '.'
                {
                    score += 80;
                }
            }

            consecutive += 1;

            query_index += 1;
        } else {
            consecutive = 0;
        }
    }

    if query_index ==
        query_chars.len()
    {
        Some(score)
    } else {
        None
    }
}