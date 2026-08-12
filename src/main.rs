use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    os::fd::AsRawFd,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use calloop::{channel, EventLoop};
use calloop_wayland_source::WaylandSource;

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use image::imageops::FilterType;

use rayon::prelude::*;

use serde::Deserialize;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor,
    delegate_keyboard,
    delegate_layer,
    delegate_output,
    delegate_registry,
    delegate_seat,
    delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{
            KeyEvent,
            KeyboardHandler,
            Keysym,
            Modifiers,
            RawModifiers,
        },
        Capability,
        SeatHandler,
        SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor,
            KeyboardInteractivity,
            Layer,
            LayerShell,
            LayerShellHandler,
            LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};

use wayland_client::{
    globals::registry_queue_init,
    protocol::{
        wl_keyboard,
        wl_output,
        wl_seat,
        wl_surface,
    },
    Connection,
    QueueHandle,
};


// ============================================================================
// CONFIG
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct Config {
    width: u32,
    height: u32,

    anchor: String,
    margin_top: u32,

    padding: u32,
    row_height: u32,
    max_results: usize,

    corner_radius: u32,
    border_width: u32,

    icon_size: u32,
    icon_gap: u32,

    font_path: Option<String>,
    font_size: f32,
    prompt_font_size: f32,

    background: String,
    opacity: f32,

    foreground: String,
    prompt_color: String,

    selected_bg: String,
    selected_fg: String,

    border_color: String,
    search_background: String,

    show_icons: bool,
    show_hint: bool,

    terminal: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            width: 620,
            height: 460,

            anchor: "center".to_string(),
            margin_top: 100,

            padding: 18,
            row_height: 42,
            max_results: 12,

            corner_radius: 18,
            border_width: 1,

            icon_size: 28,
            icon_gap: 12,

            font_path: None,
            font_size: 17.0,
            prompt_font_size: 22.0,

            background: "#101722".to_string(),
opacity: 0.85,
            foreground: "#dce9f5ff".to_string(),
            prompt_color: "#f2f7ffff".to_string(),

            selected_bg: "#28557bcc".to_string(),
            selected_fg: "#ffffffff".to_string(),

            border_color: "#6fc9ffff".to_string(),
            search_background: "#ffffff12".to_string(),

            show_icons: true,
            show_hint: true,

            terminal: "foot".to_string(),
        }
    }
}


// ============================================================================
// DEFAULT CONFIG
// ============================================================================

const DEFAULT_CONFIG_TOML: &str = r##"# waylaunch configuration

width = 620
height = 460

anchor = "center"
margin_top = 100

padding = 18
row_height = 42
max_results = 12

corner_radius = 18
border_width = 1

show_icons = true
icon_size = 28
icon_gap = 12

font_path = ""

font_size = 17.0
prompt_font_size = 22.0

background   = "#101722f5"
foreground   = "#dce9f5ff"
prompt_color = "#f2f7ffff"

selected_bg = "#28557bcc"
selected_fg = "#ffffffff"

border_color = "#6fc9ffff"
search_background = "#ffffff12"

show_hint = true

terminal = "foot"
"##;


// ============================================================================
// CONFIG PATH
// ============================================================================

fn config_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join(".config"))
        })
        .unwrap_or_else(|| PathBuf::from(".config"));

    base.join("raix").join("config.toml")
}


fn load_config() -> Config {
    let path = config_path();

    if let Ok(text) = fs::read_to_string(&path) {
        match toml::from_str::<Config>(&text) {
            Ok(mut cfg) => {
                if cfg
                    .font_path
                    .as_ref()
                    .is_some_and(|p| p.trim().is_empty())
                {
                    cfg.font_path = None;
                }

                return cfg;
            }

            Err(e) => {
                eprintln!(
                    "waylaunch: invalid config {:?}: {}",
                    path,
                    e
                );
            }
        }
    } else {
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!(
                    "waylaunch: cannot create config directory {:?}: {}",
                    parent,
                    e
                );
            }
        }

        if let Err(e) = fs::write(&path, DEFAULT_CONFIG_TOML) {
            eprintln!(
                "waylaunch: cannot write config {:?}: {}",
                path,
                e
            );
        }
    }

    Config::default()
}


// ============================================================================
// SINGLE INSTANCE
// ============================================================================

struct InstanceLock {
    _file: File,
}

impl InstanceLock {
    fn acquire() -> Option<Self> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));

        let path = runtime.join("waylaunch.lock");

        let file = match OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
        {
            Ok(file) => file,

            Err(e) => {
                eprintln!(
                    "waylaunch: cannot open lock {:?}: {}",
                    path,
                    e
                );

                return None;
            }
        };

        let result = unsafe {
            libc::flock(
                file.as_raw_fd(),
                libc::LOCK_EX | libc::LOCK_NB,
            )
        };

        if result != 0 {
            eprintln!(
                "waylaunch: another instance is already running"
            );

            return None;
        }

        Some(Self { _file: file })
    }
}


// ============================================================================
// COLORS
// ============================================================================

fn parse_hex(value: &str) -> [u8; 4] {
    let s = value.trim().trim_start_matches('#');

    fn byte(s: &str, start: usize) -> u8 {
        if start + 2 > s.len() {
            return 255;
        }

        u8::from_str_radix(
            &s[start..start + 2],
            16,
        )
        .unwrap_or(255)
    }

    match s.len() {
        6 => [
            byte(s, 0),
            byte(s, 2),
            byte(s, 4),
            255,
        ],

        8 => [
            byte(s, 0),
            byte(s, 2),
            byte(s, 4),
            byte(s, 6),
        ],

        _ => [
            255,
            255,
            255,
            255,
        ],
    }
}


fn rgb(c: [u8; 4]) -> [u8; 3] {
    [c[0], c[1], c[2]]
}


#[derive(Clone)]
struct Theme {
    background: [u8; 4],
    foreground: [u8; 3],
    prompt_color: [u8; 3],

    selected_bg: [u8; 4],
    selected_fg: [u8; 3],

    border_color: [u8; 4],
    search_background: [u8; 4],
}


impl Theme {
    fn from_config(config: &Config) -> Self {
        let mut background =
            parse_hex(&config.background);

        background[3] =
            (config.opacity.clamp(0.0, 1.0) * 255.0)
                .round() as u8;

        Self {
            background,

            foreground:
                rgb(parse_hex(&config.foreground)),

            prompt_color:
                rgb(parse_hex(&config.prompt_color)),

            selected_bg:
                parse_hex(&config.selected_bg),

            selected_fg:
                rgb(parse_hex(&config.selected_fg)),

            border_color:
                parse_hex(&config.border_color),

            search_background:
                parse_hex(&config.search_background),
        }
    }
}

// ============================================================================
// ICON
// ============================================================================

#[derive(Clone)]
struct Icon {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}


impl Icon {
    fn load(path: &Path, size: u32) -> Option<Self> {
        let image = image::open(path).ok()?;

        let image = image.resize(
            size,
            size,
            FilterType::Lanczos3,
        );

        let rgba = image.to_rgba8();

        Some(Self {
            width: rgba.width(),
            height: rgba.height(),
            pixels: rgba.into_raw(),
        })
    }
}


// ============================================================================
// ICON INDEX
// ============================================================================

struct IconIndex {
    icons: HashMap<String, PathBuf>,
}


impl IconIndex {
    fn new() -> Self {
        Self {
            icons: HashMap::new(),
        }
    }


    fn insert_if_better(
        &mut self,
        name: String,
        path: PathBuf,
        requested_size: u32,
    ) {
        if let Some(old) = self.icons.get(&name) {
            let old_score =
                icon_score(old, requested_size);

            let new_score =
                icon_score(&path, requested_size);

            if new_score >= old_score {
                return;
            }
        }

        self.icons.insert(name, path);
    }


    fn get(
        &self,
        icon_name: &str,
    ) -> Option<&PathBuf> {
        self.icons.get(
            &icon_name.to_lowercase()
        )
    }
}


// ============================================================================
// XDG DIRECTORIES
// ============================================================================

fn data_dirs() -> Vec<PathBuf> {
    let mut result = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        result.push(
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("applications"),
        );
    }

    let dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| {
            "/usr/local/share:/usr/share".to_string()
        });

    for dir in dirs.split(':') {
        if !dir.is_empty() {
            result.push(
                PathBuf::from(dir)
                    .join("applications"),
            );
        }
    }

    result
}


// ============================================================================
// ICON DIRECTORIES
// ============================================================================

fn icon_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);

        roots.push(home.join(".icons"));

        roots.push(
            home.join(".local")
                .join("share")
                .join("icons"),
        );
    }

    if let Some(data_home) =
        std::env::var_os("XDG_DATA_HOME")
    {
        roots.push(
            PathBuf::from(data_home)
                .join("icons"),
        );
    }

    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| {
            "/usr/local/share:/usr/share".to_string()
        });

    for dir in data_dirs.split(':') {
        if !dir.is_empty() {
            roots.push(
                PathBuf::from(dir)
                    .join("icons"),
            );
        }
    }

    roots
}


fn icon_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "png"
                    | "jpg"
                    | "jpeg"
                    | "bmp"
                    | "ico"
            )
        })
        .unwrap_or(false)
}


fn icon_score(
    path: &Path,
    requested: u32,
) -> u32 {
    if let Some(parent) = path.parent() {
        if let Some(name) =
            parent
                .file_name()
                .and_then(|x| x.to_str())
        {
            if let Ok(n) = name.parse::<u32>() {
                return n.abs_diff(requested);
            }
        }
    }

    1000
}


// ============================================================================
// BUILD ICON INDEX
// ============================================================================

fn build_icon_index(
    requested_size: u32,
) -> IconIndex {
    let mut index = IconIndex::new();

    let mut visited = 0usize;

    for root in icon_roots() {
        if !root.exists() {
            continue;
        }

        let mut stack = vec![(root, 0usize)];

        while let Some((dir, depth)) =
            stack.pop()
        {
            if depth > 7 {
                continue;
            }

            if visited > 60_000 {
                eprintln!(
                    "waylaunch: icon scan safety limit reached"
                );

                return index;
            }

            visited += 1;

            let Ok(entries) =
                fs::read_dir(&dir)
            else {
                continue;
            };

            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_dir() {
                    stack.push((
                        path,
                        depth + 1,
                    ));

                    continue;
                }

                if !icon_extension(&path) {
                    continue;
                }

                let Some(stem) = path
                    .file_stem()
                    .and_then(|x| x.to_str())
                else {
                    continue;
                };

                let key =
                    stem.to_lowercase();

                index.insert_if_better(
                    key,
                    path,
                    requested_size,
                );
            }
        }
    }

    index
}


// ============================================================================
// DESKTOP APPLICATION
// ============================================================================

#[derive(Clone)]
struct AppEntry {
    name: String,
    exec: String,
    terminal: bool,

    // Only path is stored.
    // Actual icon is loaded lazily.
    icon_path: Option<PathBuf>,

    haystack: String,
}


// ============================================================================
// DESKTOP PARSER
// ============================================================================

fn parse_bool(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
}


fn resolve_icon(
    icon_name: &str,
    _requested_size: u32,
    index: &IconIndex,
) -> Option<PathBuf> {
    let icon_name = icon_name.trim();

    if icon_name.is_empty() {
        return None;
    }

    // Absolute path.
    let direct =
        PathBuf::from(icon_name);

    if direct.is_absolute()
        && direct.is_file()
    {
        return Some(direct);
    }

    // Exact index lookup.
    if let Some(path) =
        index.get(icon_name)
    {
        return Some(path.clone());
    }

    // Filename without extension.
    let name = Path::new(icon_name)
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or(icon_name);

    let stem = Path::new(name)
        .file_stem()
        .and_then(|x| x.to_str())
        .unwrap_or(name);

    if let Some(path) =
        index.get(stem)
    {
        return Some(path.clone());
    }

    // Common fallback paths.
    let common = [
        "/usr/share/pixmaps",
        "/usr/local/share/pixmaps",
    ];

    for root in common {
        for ext in [
            "png",
            "jpg",
            "jpeg",
            "xpm",
            "ico",
        ] {
            let candidate =
                PathBuf::from(root)
                    .join(format!(
                        "{}.{}",
                        stem,
                        ext
                    ));

            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}


fn parse_desktop_file(
    path: &Path,
    icon_size: u32,
    icon_index: &IconIndex,
) -> Option<AppEntry> {
    let content =
        fs::read_to_string(path).ok()?;

    let mut desktop_entry = false;

    let mut app_type = String::new();

    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut icon_name: Option<String> = None;

    let mut generic_name =
        String::new();

    let mut keywords =
        String::new();

    let mut categories =
        String::new();

    let mut terminal = false;
    let mut no_display = false;
    let mut hidden = false;

    for raw in content.lines() {
        let line = raw.trim();

        if line.starts_with('[') {
            desktop_entry =
                line == "[Desktop Entry]";

            continue;
        }

        if !desktop_entry
            || line.is_empty()
            || line.starts_with('#')
        {
            continue;
        }

        let Some((key, value)) =
            line.split_once('=')
        else {
            continue;
        };

        let key = key.trim();
        let value = value.trim();

        match key {
            "Type" => {
                app_type =
                    value.to_string();
            }

            "Name" => {
                if !value.is_empty() {
                    name =
                        Some(value.to_string());
                }
            }

            "Exec" => {
                exec =
                    Some(value.to_string());
            }

            "Icon" => {
                icon_name =
                    Some(value.to_string());
            }

            "Terminal" => {
                terminal =
                    parse_bool(value);
            }

            "NoDisplay" => {
                no_display =
                    parse_bool(value);
            }

            "Hidden" => {
                hidden =
                    parse_bool(value);
            }

            "GenericName" => {
                generic_name =
                    value.to_string();
            }

            "Keywords" => {
                keywords =
                    value.to_string();
            }

            "Categories" => {
                categories =
                    value.to_string();
            }

            _ => {}
        }
    }

    if app_type != "Application" {
        return None;
    }

    if no_display || hidden {
        return None;
    }

    let name = name?;
    let exec = exec?;

    if exec.trim().is_empty() {
        return None;
    }

    let exec = clean_exec(&exec);

    if exec.is_empty() {
        return None;
    }

    let icon_path = icon_name
        .as_deref()
        .and_then(|x| {
            resolve_icon(
                x,
                icon_size,
                icon_index,
            )
        });

    let haystack = format!(
        "{} {} {} {}",
        name.to_lowercase(),
        generic_name.to_lowercase(),
        keywords.to_lowercase(),
        categories.to_lowercase(),
    );

    Some(AppEntry {
        name,
        exec,
        terminal,
        icon_path,
        haystack,
    })
}


// ============================================================================
// EXEC CLEANUP
// ============================================================================

fn clean_exec(exec: &str) -> String {
    let mut result = Vec::new();

    for token in exec.split_whitespace() {
        if token.starts_with('%') {
            continue;
        }

        result.push(token);
    }

    result.join(" ")
}


// ============================================================================
// SCAN APPLICATIONS
// ============================================================================

fn scan_apps(
    icon_size: u32,
) -> Vec<AppEntry> {
    eprintln!(
        "waylaunch: background scan started..."
    );

    let icon_index =
        build_icon_index(icon_size);

    eprintln!(
        "waylaunch: icon index ready ({} icons)",
        icon_index.icons.len()
    );

    let dirs = data_dirs();

    let mut seen_files =
        HashSet::<String>::new();

    let mut files =
        Vec::<PathBuf>::new();

    for dir in &dirs {
        let Ok(entries) =
            fs::read_dir(dir)
        else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();

            let is_desktop = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| {
                    e.eq_ignore_ascii_case(
                        "desktop"
                    )
                });

            if !is_desktop {
                continue;
            }

            let Some(filename) =
                path.file_name()
                    .and_then(|x| x.to_str())
            else {
                continue;
            };

            if seen_files.insert(
                filename.to_string()
            ) {
                files.push(path);
            }
        }
    }

    eprintln!(
        "waylaunch: parsing {} desktop files...",
        files.len()
    );

    let mut apps: Vec<AppEntry> =
        files
            .par_iter()
            .filter_map(|path| {
                parse_desktop_file(
                    path,
                    icon_size,
                    &icon_index,
                )
            })
            .collect();

    let mut names =
        HashSet::<String>::new();

    apps.retain(|app| {
        names.insert(
            app.name.to_lowercase()
        )
    });

    apps.sort_by_cached_key(|app| {
        app.name.to_lowercase()
    });

    eprintln!(
        "waylaunch: scan finished: {} apps",
        apps.len()
    );

    apps
}


// ============================================================================
// FUZZY SEARCH
// ============================================================================

fn fuzzy_filter(
    apps: &[AppEntry],
    query: &str,
    matcher: &SkimMatcherV2,
    max_results: usize,
) -> Vec<AppEntry> {
    if query.trim().is_empty() {
        return apps
            .iter()
            .take(max_results)
            .cloned()
            .collect();
    }

    let query =
        query.to_lowercase();

    let mut scored =
        Vec::<(i64, &AppEntry)>::new();

    for app in apps {
        if let Some(score) =
            matcher.fuzzy_match(
                &app.haystack,
                &query,
            )
        {
            scored.push((
                score,
                app,
            ));
        }
    }

    scored.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| {
                a.1.name
                    .to_lowercase()
                    .cmp(
                        &b.1.name
                            .to_lowercase()
                    )
            })
    });

    scored
        .into_iter()
        .take(max_results)
        .map(|(_, app)| app.clone())
        .collect()
}


// ============================================================================
// APP LAUNCH
// ============================================================================

fn launch_app(
    app: &AppEntry,
    config: &Config,
) {
    let exec =
        app.exec.trim();

    if exec.is_empty() {
        return;
    }

    let mut command;

    if app.terminal {
        let terminal =
            std::env::var("TERMINAL")
                .unwrap_or_else(|_| {
                    config.terminal.clone()
                });

        command =
            Command::new(&terminal);

        command
            .arg("-e")
            .arg("sh")
            .arg("-c")
            .arg(exec);
    } else {
        command =
            Command::new("sh");

        command
            .arg("-c")
            .arg(exec);
    }

    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());

    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    if let Err(e) =
        command.spawn()
    {
        eprintln!(
            "waylaunch: failed to launch '{}': {}",
            app.name,
            e
        );
    }
}


// ============================================================================
// FONT
// ============================================================================

fn load_font(
    preferred: Option<&str>,
) -> fontdue::Font {
    if let Some(path) = preferred {
        match fs::read(path) {
            Ok(bytes) => {
                match fontdue::Font::from_bytes(
                    bytes,
                    fontdue::FontSettings::default(),
                ) {
                    Ok(font) => return font,

                    Err(e) => {
                        eprintln!(
                            "waylaunch: invalid font {:?}: {}",
                            path,
                            e
                        );
                    }
                }
            }

            Err(e) => {
                eprintln!(
                    "waylaunch: cannot read font {:?}: {}",
                    path,
                    e
                );
            }
        }
    }

    const FONTS: &[&str] = &[
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/liberation2/LiberationSans-Regular.ttf",
    ];

    for path in FONTS {
        if let Ok(bytes) =
            fs::read(path)
        {
            if let Ok(font) =
                fontdue::Font::from_bytes(
                    bytes,
                    fontdue::FontSettings::default(),
                )
            {
                return font;
            }
        }
    }

    panic!(
        "waylaunch: no usable TTF font found. \
         Install DejaVu Sans."
    );
}


// ============================================================================
// DRAWING
// ============================================================================

fn blend_pixel(
    canvas: &mut [u8],
    stride_px: i32,
    x: i32,
    y: i32,
    color: [u8; 3],
    alpha: u8,
) {
    if x < 0
        || y < 0
        || x >= stride_px
    {
        return;
    }

    let index =
        ((y * stride_px + x) * 4) as usize;

    if index + 4 > canvas.len() {
        return;
    }

    let sa = alpha as u32;
    let da = canvas[index + 3] as u32;

    if sa == 0 {
        return;
    }

    // Source-over alpha compositing.
    let out_a =
        sa + (da * (255 - sa) + 127) / 255;

    if out_a == 0 {
        return;
    }

    // Wayland ARGB8888 on little-endian systems is stored as BGRA.
    let src_b = color[2] as u32;
    let src_g = color[1] as u32;
    let src_r = color[0] as u32;

    let dst_b = canvas[index] as u32;
    let dst_g = canvas[index + 1] as u32;
    let dst_r = canvas[index + 2] as u32;

    let inv_sa = 255 - sa;

    let out_b =
        (src_b * sa
            + dst_b * da * inv_sa / 255)
            / out_a;

    let out_g =
        (src_g * sa
            + dst_g * da * inv_sa / 255)
            / out_a;

    let out_r =
        (src_r * sa
            + dst_r * da * inv_sa / 255)
            / out_a;

    canvas[index] =
        out_b.min(255) as u8;

    canvas[index + 1] =
        out_g.min(255) as u8;

    canvas[index + 2] =
        out_r.min(255) as u8;

    canvas[index + 3] =
        out_a.min(255) as u8;
}

fn fill_canvas(
    canvas: &mut [u8],
    width: i32,
    height: i32,
    color: [u8; 4],
) {
    let pixel = [
        color[2],
        color[1],
        color[0],
        color[3],
    ];

    for y in 0..height {
        for x in 0..width {
            let index =
                ((y * width + x) * 4)
                    as usize;

            canvas[index..index + 4]
                .copy_from_slice(&pixel);
        }
    }
}


fn rounded_rect_contains(
    px: i32,
    py: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    radius: i32,
) -> bool {
    if width <= 0
        || height <= 0
    {
        return false;
    }

    let local_x = px - x;
    let local_y = py - y;

    if local_x < 0
        || local_y < 0
        || local_x >= width
        || local_y >= height
    {
        return false;
    }

    let r = radius
        .min(width / 2)
        .min(height / 2)
        .max(0);

    if r == 0 {
        return true;
    }

    let left = local_x < r;
    let right = local_x >= width - r;
    let top = local_y < r;
    let bottom = local_y >= height - r;

    if !(left || right)
        || !(top || bottom)
    {
        return true;
    }

    let cx = if left {
        r
    } else {
        width - r - 1
    };

    let cy = if top {
        r
    } else {
        height - r - 1
    };

    let dx = local_x - cx;
    let dy = local_y - cy;

    dx * dx + dy * dy <= r * r
}


fn draw_rounded_rect(
    canvas: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    radius: i32,
    color: [u8; 4],
) {
    if w <= 0
        || h <= 0
    {
        return;
    }

    let x0 = x.max(0);
    let y0 = y.max(0);
    let x1 = (x + w).min(width);
    let y1 = (y + h).min(height);

    if x0 >= x1
        || y0 >= y1
    {
        return;
    }

    for py in y0..y1 {
        for px in x0..x1 {
            if rounded_rect_contains(
                px,
                py,
                x,
                y,
                w,
                h,
                radius,
            ) {
                blend_pixel(
                    canvas,
                    width,
                    px,
                    py,
                    rgb(color),
                    color[3],
                );
            }
        }
    }
}


fn draw_border(
    canvas: &mut [u8],
    width: i32,
    height: i32,
    radius: i32,
    border_width: i32,
    color: [u8; 4],
) {
    if border_width <= 0 {
        return;
    }

    let inner_w =
        width - border_width * 2;

    let inner_h =
        height - border_width * 2;

    let inner_radius =
        (radius - border_width)
            .max(0);

    for py in 0..height {
        for px in 0..width {
            if !rounded_rect_contains(
                px,
                py,
                0,
                0,
                width,
                height,
                radius,
            ) {
                continue;
            }

            let inside =
                inner_w > 0
                    && inner_h > 0
                    && rounded_rect_contains(
                        px,
                        py,
                        border_width,
                        border_width,
                        inner_w,
                        inner_h,
                        inner_radius,
                    );

            if !inside {
                blend_pixel(
                    canvas,
                    width,
                    px,
                    py,
                    rgb(color),
                    color[3],
                );
            }
        }
    }
}


fn draw_text(
    canvas: &mut [u8],
    width: i32,
    height: i32,
    x: i32,
    baseline: i32,
    text: &str,
    font: &fontdue::Font,
    size: f32,
    color: [u8; 3],
) {
    let mut cursor =
        x as f32;

    for ch in text.chars() {
        let (metrics, bitmap) =
            font.rasterize(ch, size);

        let gx0 =
            cursor.round() as i32
                + metrics.xmin;

        let gy0 =
            baseline
                - metrics.height as i32
                - metrics.ymin;

        for gy in 0..metrics.height {
            for gx in 0..metrics.width {
                let alpha =
                    bitmap[
                        gy * metrics.width
                            + gx
                    ];

                if alpha == 0 {
                    continue;
                }

                let px =
                    gx0 + gx as i32;

                let py =
                    gy0 + gy as i32;

                if px < 0
                    || py < 0
                    || px >= width
                    || py >= height
                {
                    continue;
                }

                blend_pixel(
                    canvas,
                    width,
                    px,
                    py,
                    color,
                    alpha,
                );
            }
        }

        cursor +=
            metrics.advance_width;
    }
}


// ============================================================================
// ICON DRAW
// ============================================================================

fn draw_icon(
    canvas: &mut [u8],
    width: i32,
    height: i32,
    icon: &Icon,
    x: i32,
    y: i32,
    size: i32,
) {
    if size <= 0 {
        return;
    }

    let iw =
        icon.width as i32;

    let ih =
        icon.height as i32;

    if iw <= 0
        || ih <= 0
    {
        return;
    }

    for dy in 0..size {
        for dx in 0..size {
            let sx =
                (dx * iw / size)
                    .clamp(0, iw - 1);

            let sy =
                (dy * ih / size)
                    .clamp(0, ih - 1);

            let source =
                ((sy * iw + sx) * 4)
                    as usize;

            if source + 4 >
                icon.pixels.len()
            {
                continue;
            }

            let r =
                icon.pixels[source];

            let g =
                icon.pixels[source + 1];

            let b =
                icon.pixels[source + 2];

            let a =
                icon.pixels[source + 3];

            if a == 0 {
                continue;
            }

            let px =
                x + dx;

            let py =
                y + dy;

            if px < 0
                || py < 0
                || px >= width
                || py >= height
            {
                continue;
            }

            blend_pixel(
                canvas,
                width,
                px,
                py,
                [r, g, b],
                a,
            );
        }
    }
}


// ============================================================================
// LAUNCHER
// ============================================================================

struct Launcher {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,

    shm: Shm,
    pool: SlotPool,

    layer: LayerSurface,

    keyboard:
        Option<wl_keyboard::WlKeyboard>,

    config: Config,
    theme: Theme,

    font: fontdue::Font,
    matcher: SkimMatcherV2,

    all_apps: Vec<AppEntry>,
    filtered: Vec<AppEntry>,

    query: String,
    selected: usize,

    // First visible item.
    scroll_offset: usize,

    width: i32,
    height: i32,

    configured: bool,
    dirty: bool,
    exit: bool,

    apps_loading: bool,

    // Lazy icon cache.
    icon_cache: HashMap<PathBuf, Icon>,
}


impl Launcher {
    // ----------------------------------------------------------------
    // Visible rows
    // ----------------------------------------------------------------

    fn visible_rows(&self) -> usize {
        let padding =
            self.config.padding as i32;

        let search_height = 54i32;

        let row_height =
            self.config.row_height as i32;

        let list_top =
            padding
                + search_height
                + 12;

        let footer =
            if self.config.show_hint {
                25
            } else {
                5
            };

        let available =
            self.height
                - padding
                - footer
                - list_top;

        if available <= 0
            || row_height <= 0
        {
            return 1;
        }

        (available / row_height)
            .max(1) as usize
    }


    // ----------------------------------------------------------------
    // Keep selection visible
    // ----------------------------------------------------------------

    fn ensure_selection_visible(
        &mut self,
    ) {
        let visible =
            self.visible_rows();

        if visible == 0 {
            self.scroll_offset = 0;
            return;
        }

        if self.selected <
            self.scroll_offset
        {
            self.scroll_offset =
                self.selected;
        }

        let bottom =
            self.scroll_offset
                + visible;

        if self.selected >= bottom {
            self.scroll_offset =
                self.selected + 1 - visible;
        }

        let max_scroll =
            self.filtered
                .len()
                .saturating_sub(visible);

        if self.scroll_offset >
            max_scroll
        {
            self.scroll_offset =
                max_scroll;
        }
    }


    fn refilter(&mut self) {
        self.filtered =
            fuzzy_filter(
                &self.all_apps,
                &self.query,
                &self.matcher,
                self.config.max_results,
            );

        if self.filtered.is_empty() {
            self.selected = 0;
            self.scroll_offset = 0;
        } else {
            if self.selected
                >= self.filtered.len()
            {
                self.selected =
                    self.filtered.len() - 1;
            }

            self.scroll_offset = 0;

            self.ensure_selection_visible();
        }

        self.dirty = true;
    }


    fn set_apps(
        &mut self,
        apps: Vec<AppEntry>,
    ) {
        self.all_apps = apps;

        self.apps_loading = false;

        self.selected = 0;
        self.scroll_offset = 0;

        self.refilter();

        self.dirty = true;
    }


    // ----------------------------------------------------------------
    // Lazy icon loader
    //
    // NOTE:
    // This is called BEFORE create_buffer().
    // Therefore it cannot conflict with the SHM canvas borrow.
    // ----------------------------------------------------------------

    fn preload_visible_icons(
        &mut self,
        start: usize,
        end: usize,
    ) {
        if !self.config.show_icons {
            return;
        }

        let paths: Vec<PathBuf> =
            self.filtered[start..end]
                .iter()
                .filter_map(|app| {
                    app.icon_path.clone()
                })
                .collect();

        let icon_size =
            self.config.icon_size;

        for path in paths {
            if self.icon_cache.contains_key(&path) {
                continue;
            }

            if let Some(icon) =
                Icon::load(
                    &path,
                    icon_size,
                )
            {
                self.icon_cache.insert(
                    path,
                    icon,
                );
            }
        }
    }


    // ----------------------------------------------------------------
    // DRAW
    //
    // IMPORTANT BORROW FIX:
    //
    // Everything that needs `&mut self` is done BEFORE
    // create_buffer().
    //
    // Once canvas is borrowed from self.pool, we don't call
    // self.visible_rows(), self.get_icon(), etc.
    // ----------------------------------------------------------------

    fn draw(
        &mut self,
        _qh: &QueueHandle<Self>,
    ) {
        if !self.configured {
            return;
        }

        if self.width <= 0
            || self.height <= 0
        {
            return;
        }

        let width =
            self.width;

        let height =
            self.height;

        let stride =
            width * 4;

        // ============================================================
        // PRE-CALCULATE EVERYTHING BEFORE SHM BORROW
        // ============================================================

        let padding =
            self.config.padding as i32;

        let search_height =
            54i32;

        let row_height =
            self.config.row_height as i32;

        let list_top =
            padding
                + search_height
                + 12;

        let visible =
            self.visible_rows();

        let start =
            self.scroll_offset
                .min(self.filtered.len());

        let end =
            (start + visible)
                .min(self.filtered.len());

        // Load visible icons BEFORE create_buffer().
        self.preload_visible_icons(
            start,
            end,
        );

        // Copy configuration values locally.
        let radius =
            self.config.corner_radius as i32;

        let border =
            self.config.border_width as i32;

        let icon_size =
            self.config.icon_size as i32;

        let icon_gap =
            self.config.icon_gap as i32;

        let show_icons =
            self.config.show_icons;

        let show_hint =
            self.config.show_hint;

        let font_size =
            self.config.font_size;

        let prompt_font_size =
            self.config.prompt_font_size;

        let background =
            self.theme.background;

        let foreground =
            self.theme.foreground;

        let prompt_color =
            self.theme.prompt_color;

        let selected_bg =
            self.theme.selected_bg;

        let selected_fg =
            self.theme.selected_fg;

        let border_color =
            self.theme.border_color;

        let search_background =
            self.theme.search_background;

        let query =
            self.query.clone();

        let apps_loading =
            self.apps_loading;

        // ============================================================
        // COPY VISIBLE APP DATA
        //
        // This is important because after create_buffer() we don't
        // want to create conflicting borrows into self.filtered.
        // ============================================================

        let visible_apps: Vec<(
            usize,
            String,
            Option<Icon>,
        )> = self.filtered[start..end]
            .iter()
            .enumerate()
            .map(|(offset, app)| {
                let index =
                    start + offset;

                let icon =
                    if show_icons {
                        app.icon_path
                            .as_ref()
                            .and_then(|path| {
                                self.icon_cache
                                    .get(path)
                                    .cloned()
                            })
                    } else {
                        None
                    };

                (
                    index,
                    app.name.clone(),
                    icon,
                )
            })
            .collect();

        // ============================================================
        // NOW CREATE SHM BUFFER
        //
        // From here onward, do NOT call methods on self that require
        // borrowing self.
        // ============================================================

        let (buffer, canvas) =
            match self.pool.create_buffer(
                width,
                height,
                stride,
                wayland_client::protocol::wl_shm::Format::Argb8888,
            ) {
                Ok(value) => value,

                Err(e) => {
                    eprintln!(
                        "waylaunch: SHM buffer creation failed: {}",
                        e
                    );

                    return;
                }
            };

        // ============================================================
        // BACKGROUND
        // ============================================================

        fill_canvas(
            canvas,
            width,
            height,
            [0, 0, 0, 0],
        );

        // ============================================================
        // PANEL
        // ============================================================

        draw_rounded_rect(
            canvas,
            width,
            height,
            0,
            0,
            width,
            height,
            radius,
            background,
        );

        draw_border(
            canvas,
            width,
            height,
            radius,
            border,
            border_color,
        );

        // ============================================================
        // SEARCH BOX
        // ============================================================

        let search_x =
            padding;

        let search_y =
            padding;

        let search_width =
            width - padding * 2;

        draw_rounded_rect(
            canvas,
            width,
            height,
            search_x,
            search_y,
            search_width,
            search_height,
            12,
            search_background,
        );

        let prompt =
            if query.is_empty() {
                if apps_loading {
                    "> Loading applications..."
                        .to_string()
                } else {
                    "> Search applications"
                        .to_string()
                }
            } else {
                format!(
                    "> {}",
                    query
                )
            };

        draw_text(
            canvas,
            width,
            height,
            padding + 14,
            padding + 37,
            &prompt,
            &self.font,
            prompt_font_size,
            prompt_color,
        );

        // ============================================================
        // RESULTS
        // ============================================================

        for (
            index,
            app_name,
            icon,
        ) in visible_apps
        {
            let visual_index =
                index - start;

            let row_y =
                list_top
                    + visual_index as i32
                        * row_height;

            let selected =
                index == self.selected;

            // --------------------------------------------------------
            // Selected background
            // --------------------------------------------------------

            if selected {
                draw_rounded_rect(
                    canvas,
                    width,
                    height,
                    padding,
                    row_y,
                    width - padding * 2,
                    row_height - 3,
                    10,
                    selected_bg,
                );

                draw_rounded_rect(
                    canvas,
                    width,
                    height,
                    padding + 7,
                    row_y + 9,
                    4,
                    row_height - 21,
                    2,
                    [120, 210, 255, 220],
                );
            }

            let text_color =
                if selected {
                    selected_fg
                } else {
                    foreground
                };

            let mut text_x =
                padding + 22;

            // --------------------------------------------------------
            // ICON
            // --------------------------------------------------------

            if let Some(icon) =
                icon.as_ref()
            {
                let icon_y =
                    row_y
                        + (row_height
                            - icon_size)
                            / 2;

                draw_icon(
                    canvas,
                    width,
                    height,
                    icon,
                    padding + 18,
                    icon_y,
                    icon_size,
                );

                text_x =
                    padding
                        + 18
                        + icon_size
                        + icon_gap;
            }

            // --------------------------------------------------------
            // APP NAME
            // --------------------------------------------------------

            draw_text(
                canvas,
                width,
                height,
                text_x,
                row_y
                    + row_height
                    - 11,
                &app_name,
                &self.font,
                font_size,
                text_color,
            );
        }

        // ============================================================
        // SCROLL INDICATOR
        // ============================================================

        let filtered_len =
            self.filtered.len();

        if filtered_len > visible {
            let track_h =
                (visible as i32
                    * row_height)
                    .max(1);

            let thumb_h =
                ((visible as f32
                    / filtered_len as f32)
                    * track_h as f32)
                    .max(12.0) as i32;

            let max_scroll =
                filtered_len
                    .saturating_sub(visible);

            let thumb_y =
                if max_scroll == 0 {
                    0
                } else {
                    ((self.scroll_offset
                        as f32
                        / max_scroll as f32)
                        * (track_h - thumb_h)
                        as f32) as i32
                };

            draw_rounded_rect(
                canvas,
                width,
                height,
                width - padding - 4,
                list_top + thumb_y,
                3,
                thumb_h,
                2,
                [120, 210, 255, 150],
            );
        }

        // ============================================================
        // EMPTY STATE
        // ============================================================

        if !apps_loading
            && filtered_len == 0
        {
            draw_text(
                canvas,
                width,
                height,
                padding + 12,
                list_top + 38,
                "No applications found",
                &self.font,
                15.0,
                [145, 165, 185],
            );
        }

        // ============================================================
        // FOOTER
        // ============================================================

        if apps_loading {
            draw_text(
                canvas,
                width,
                height,
                padding,
                height - 12,
                "Loading applications...",
                &self.font,
                11.0,
                [135, 155, 175],
            );
        } else if show_hint {
            draw_text(
                canvas,
                width,
                height,
                padding,
                height - 12,
                "↑ ↓  Navigate    Enter  Launch    Esc  Close",
                &self.font,
                11.0,
                [135, 155, 175],
            );
        }

        // ============================================================
        // COMMIT
        // ============================================================

        let surface =
            self.layer.wl_surface();

        if let Err(e) =
            buffer.attach_to(surface)
        {
            eprintln!(
                "waylaunch: buffer attach failed: {}",
                e
            );

            return;
        }

        surface.damage_buffer(
            0,
            0,
            width,
            height,
        );

        self.layer.commit();

        self.dirty = false;
    }


    // ----------------------------------------------------------------
    // SELECTION
    // ----------------------------------------------------------------

    fn move_selection_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;

            self.ensure_selection_visible();

            self.dirty = true;
        }
    }


    fn move_selection_down(&mut self) {
        if self.selected + 1
            < self.filtered.len()
        {
            self.selected += 1;

            self.ensure_selection_visible();

            self.dirty = true;
        }
    }


    fn move_home(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = 0;
            self.scroll_offset = 0;
            self.dirty = true;
        }
    }


    fn move_end(&mut self) {
        if !self.filtered.is_empty() {
            self.selected =
                self.filtered.len() - 1;

            self.ensure_selection_visible();

            self.dirty = true;
        }
    }


    fn activate(&mut self) {
        if let Some(app) =
            self.filtered
                .get(self.selected)
                .cloned()
        {
            launch_app(
                &app,
                &self.config,
            );
        }

        self.exit = true;
    }
}


// ============================================================================
// COMPOSITOR
// ============================================================================

impl CompositorHandler for Launcher {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _factor: i32,
    ) {
        self.dirty = true;
    }


    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _transform: wl_output::Transform,
    ) {
        self.dirty = true;
    }


    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }


    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }


    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}


// ============================================================================
// OUTPUT
// ============================================================================

impl OutputHandler for Launcher {
    fn output_state(
        &mut self,
    ) -> &mut OutputState {
        &mut self.output_state
    }


    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }


    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }


    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}


// ============================================================================
// LAYER SHELL
// ============================================================================

impl LayerShellHandler for Launcher {
    fn closed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
    ) {
        self.exit = true;
    }


    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (width, height) =
            configure.new_size;

        self.width =
            if width == 0 {
                self.config.width as i32
            } else {
                width as i32
            };

        self.height =
            if height == 0 {
                self.config.height as i32
            } else {
                height as i32
            };

        if !self.configured {
            self.configured = true;

            self.refilter();
        }

        self.ensure_selection_visible();

        self.dirty = true;

        self.draw(qh);
    }
}


// ============================================================================
// SEAT
// ============================================================================

impl SeatHandler for Launcher {
    fn seat_state(
        &mut self,
    ) -> &mut SeatState {
        &mut self.seat_state
    }


    fn new_seat(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }


    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability ==
            Capability::Keyboard
            && self.keyboard.is_none()
        {
            match self.seat_state
                .get_keyboard(
                    qh,
                    &seat,
                    None,
                )
            {
                Ok(keyboard) => {
                    self.keyboard =
                        Some(keyboard);
                }

                Err(e) => {
                    eprintln!(
                        "waylaunch: failed to acquire keyboard: {}",
                        e
                    );
                }
            }
        }
    }


    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability ==
            Capability::Keyboard
        {
            self.keyboard = None;
        }
    }


    fn remove_seat(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
        self.keyboard = None;
    }
}


// ============================================================================
// KEYBOARD
// ============================================================================

impl KeyboardHandler for Launcher {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
    }


    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }


    fn press_key(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        match event.keysym {
            Keysym::Escape => {
                self.exit = true;
                return;
            }

            Keysym::Return
            | Keysym::KP_Enter => {
                self.activate();
                return;
            }

            Keysym::BackSpace => {
                if self.query.pop().is_some() {
                    self.refilter();
                }
            }

            Keysym::Up => {
                self.move_selection_up();
            }

            Keysym::Down => {
                self.move_selection_down();
            }

            Keysym::Home => {
                self.move_home();
            }

            Keysym::End => {
                self.move_end();
            }

            _ => {
                if let Some(text) =
                    event.utf8
                {
                    if !text.is_empty()
                        && text.chars().all(
                            |c| !c.is_control()
                        )
                    {
                        self.query
                            .push_str(&text);

                        self.refilter();
                    }
                }
            }
        }

        if self.dirty {
            self.draw(qh);
        }
    }


    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }


    fn repeat_key(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        match event.keysym {
            Keysym::BackSpace => {
                if self.query.pop().is_some() {
                    self.refilter();
                }
            }

            Keysym::Up => {
                self.move_selection_up();
            }

            Keysym::Down => {
                self.move_selection_down();
            }

            _ => {}
        }

        if self.dirty {
            self.draw(qh);
        }
    }


    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _raw_modifiers: RawModifiers,
        _layout: u32,
    ) {
    }
}


// ============================================================================
// SHM
// ============================================================================

impl ShmHandler for Launcher {
    fn shm_state(
        &mut self,
    ) -> &mut Shm {
        &mut self.shm
    }
}


// ============================================================================
// REGISTRY
// ============================================================================

impl ProvidesRegistryState for Launcher {
    fn registry(
        &mut self,
    ) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![
        OutputState,
        SeatState
    ];
}


// ============================================================================
// DELEGATES
// ============================================================================

delegate_compositor!(Launcher);
delegate_output!(Launcher);
delegate_shm!(Launcher);
delegate_seat!(Launcher);
delegate_keyboard!(Launcher);
delegate_layer!(Launcher);
delegate_registry!(Launcher);


// ============================================================================
// MAIN
// ============================================================================

fn main() {
    // ------------------------------------------------------------------------
    // SINGLE INSTANCE
    // ------------------------------------------------------------------------

    let Some(_instance_lock) =
        InstanceLock::acquire()
    else {
        return;
    };


    // ------------------------------------------------------------------------
    // CONFIG
    // ------------------------------------------------------------------------

    let config =
        load_config();

    let theme =
        Theme::from_config(&config);


    // ------------------------------------------------------------------------
    // WAYLAND
    // ------------------------------------------------------------------------

    let connection =
        Connection::connect_to_env()
            .expect(
                "waylaunch: cannot connect to Wayland"
            );

    // `mut` removed because event_queue itself is moved
    // into WaylandSource below.
    let (globals, event_queue) =
        registry_queue_init(
            &connection
        )
        .expect(
            "waylaunch: registry initialization failed"
        );

    let qh =
        event_queue.handle();


    // ------------------------------------------------------------------------
    // EVENT LOOP
    // ------------------------------------------------------------------------

    let mut event_loop:
        EventLoop<Launcher> =
        EventLoop::try_new()
            .expect(
                "waylaunch: event loop creation failed"
            );

    WaylandSource::new(
        connection.clone(),
        event_queue,
    )
    .insert(
        event_loop.handle()
    )
    .expect(
        "waylaunch: failed to attach Wayland event source"
    );


    // ------------------------------------------------------------------------
    // COMPOSITOR
    // ------------------------------------------------------------------------

    let compositor =
        CompositorState::bind(
            &globals,
            &qh,
        )
        .expect(
            "waylaunch: wl_compositor is missing"
        );


    // ------------------------------------------------------------------------
    // LAYER SHELL
    // ------------------------------------------------------------------------

    let layer_shell =
        LayerShell::bind(
            &globals,
            &qh,
        )
        .expect(
            "waylaunch: zwlr_layer_shell_v1 is missing. \
             Hyprland/wlr-layer-shell is required."
        );


    // ------------------------------------------------------------------------
    // SHM
    // ------------------------------------------------------------------------

    let shm =
        Shm::bind(
            &globals,
            &qh,
        )
        .expect(
            "waylaunch: wl_shm is missing"
        );


    // ------------------------------------------------------------------------
    // SURFACE
    // ------------------------------------------------------------------------

    let surface =
        compositor.create_surface(
            &qh
        );

    let layer =
        layer_shell.create_layer_surface(
            &qh,
            surface,
            Layer::Overlay,
            Some("waylaunch"),
            None,
        );


    // ------------------------------------------------------------------------
    // KEYBOARD
    // ------------------------------------------------------------------------

    layer.set_keyboard_interactivity(
        KeyboardInteractivity::Exclusive
    );


    // ------------------------------------------------------------------------
    // SIZE
    // ------------------------------------------------------------------------

    layer.set_size(
        config.width,
        config.height,
    );


    // ------------------------------------------------------------------------
    // POSITION
    // ------------------------------------------------------------------------

    if config
        .anchor
        .eq_ignore_ascii_case("top")
    {
        layer.set_anchor(
            Anchor::TOP
        );

        layer.set_margin(
            config.margin_top as i32,
            0,
            0,
            0,
        );
    } else {
        layer.set_anchor(
            Anchor::empty()
        );
    }


    // ------------------------------------------------------------------------
    // INITIAL COMMIT
    // ------------------------------------------------------------------------

    layer.commit();


    // ------------------------------------------------------------------------
    // SHM POOL
    // ------------------------------------------------------------------------

    let initial_size =
        (config.width as usize)
            .saturating_mul(
                config.height as usize
            )
            .saturating_mul(4);

    let pool_size =
        initial_size
            .saturating_mul(3)
            .max(4096);

    let pool =
        SlotPool::new(
            pool_size,
            &shm,
        )
        .expect(
            "waylaunch: SHM pool creation failed"
        );


    // ------------------------------------------------------------------------
    // FONT
    // ------------------------------------------------------------------------

    let font =
        load_font(
            config.font_path
                .as_deref()
        );


    // ------------------------------------------------------------------------
    // ASYNC APPLICATION SCAN
    // ------------------------------------------------------------------------

    let (apps_sender, apps_receiver) =
        channel::channel::<Vec<AppEntry>>();

    let icon_size =
        config.icon_size;

    thread::Builder::new()
        .name("waylaunch-app-scan".into())
        .spawn(move || {
            let apps =
                scan_apps(icon_size);

            if let Err(_) =
                apps_sender.send(apps)
            {
                eprintln!(
                    "waylaunch: UI closed before application scan finished"
                );
            }
        })
        .expect(
            "waylaunch: failed to spawn application scanner"
        );


    // ------------------------------------------------------------------------
    // STATE
    // ------------------------------------------------------------------------

    let mut launcher =
        Launcher {
            registry_state:
                RegistryState::new(
                    &globals
                ),

            seat_state:
                SeatState::new(
                    &globals,
                    &qh,
                ),

            output_state:
                OutputState::new(
                    &globals,
                    &qh,
                ),

            shm,
            pool,
            layer,

            keyboard: None,

            config,
            theme,

            font,

            matcher:
                SkimMatcherV2::default(),

            all_apps:
                Vec::new(),

            filtered:
                Vec::new(),

            query:
                String::new(),

            selected:
                0,

            scroll_offset:
                0,

            width:
                0,

            height:
                0,

            configured:
                false,

            dirty:
                false,

            exit:
                false,

            apps_loading:
                true,

            icon_cache:
                HashMap::new(),
        };


    // ------------------------------------------------------------------------
    // APP SCAN CHANNEL
    // ------------------------------------------------------------------------

    event_loop
        .handle()
        .insert_source(
            apps_receiver,
            |event, _, launcher: &mut Launcher| {
                match event {
                    channel::Event::Msg(apps) => {
                        eprintln!(
                            "waylaunch: received {} applications",
                            apps.len()
                        );

                        launcher.set_apps(apps);

                        if launcher.configured {
                            launcher.dirty = true;
                        }
                    }

                    channel::Event::Closed => {
                        if launcher.apps_loading {
                            eprintln!(
                                "waylaunch: application scanner channel closed"
                            );

                            launcher.apps_loading =
                                false;

                            launcher.dirty = true;
                        }
                    }
                }
            },
        )
        .expect(
            "waylaunch: failed to register application scan channel"
        );


    // ------------------------------------------------------------------------
    // EVENT LOOP
    // ------------------------------------------------------------------------

    while !launcher.exit {
        event_loop
            .dispatch(
                Some(Duration::from_millis(16)),
                &mut launcher,
            )
            .expect(
                "waylaunch: event loop dispatch failed"
            );

        if launcher.dirty {
            launcher.draw(&qh);
        }
    }
}