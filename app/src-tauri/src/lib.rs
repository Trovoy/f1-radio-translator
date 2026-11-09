mod engine;
mod settings;
#[cfg(test)]
mod tests;
mod translate;
mod vision;

use engine::{RunState, Shared};
use serde::{Deserialize, Serialize};
use settings::{Region, Settings, Style, WindowRect};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

type S<'a> = State<'a, Arc<Shared>>;

#[tauri::command]
fn get_settings(shared: S) -> Settings {
    shared.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(app: AppHandle, shared: S, settings: Settings) -> Result<(), String> {
    let overlay = shared.settings.lock().unwrap().overlay;
    let mut next = settings;
    next.overlay = overlay;
    *shared.settings.lock().unwrap() = next.clone();
    shared.save()?;
    let _ = app.emit("settings-changed", next);
    Ok(())
}

#[tauri::command]
fn update_style(app: AppHandle, shared: S, style: Style) -> Result<(), String> {
    shared.settings.lock().unwrap().style = style.clone();
    shared.save()?;
    let _ = app.emit("style-changed", style);
    Ok(())
}

#[derive(Serialize)]
struct Snapshot {
    state: RunState,
    locked: bool,
    ocr: String,
}

#[tauri::command]
fn get_state(shared: S) -> Snapshot {
    Snapshot {
        state: shared.run_state(),
        locked: shared.locked.load(Ordering::SeqCst),
        ocr: shared.ocr_language.lock().unwrap().clone(),
    }
}

#[tauri::command]
fn engine_cmd(app: AppHandle, shared: S, action: String) {
    let shared = shared.inner().clone();
    match action.as_str() {
        "start" => engine::start(app, shared),
        "pause" => engine::pause(&app, &shared),
        "stop" => engine::stop(&app, &shared),
        _ => engine::toggle(app, shared),
    }
}

#[derive(Serialize)]
struct TestResult {
    text: String,
    ms: u64,
}

#[tauri::command]
async fn test_provider(id: String, fields: translate::Fields) -> Result<TestResult, String> {
    let client = translate::client();
    let job = translate::Job { text: "Box this lap, we're going to the hard tyre.", context: &[], glossary: translate::GLOSSARY };
    let t0 = Instant::now();
    let text = translate::translate(&client, &id, &fields, &job).await?;
    Ok(TestResult { text, ms: t0.elapsed().as_millis() as u64 })
}

/// Returns `[w u32][h u32][RGBA…]` so the page can paint it on a canvas.
#[tauri::command]
fn capture_preview(shared: S) -> Result<tauri::ipc::Response, String> {
    let region = shared.settings.lock().unwrap().region.ok_or("尚未框选区域")?;
    let frame = vision::capture(&region)?;
    let mut out = Vec::with_capacity(8 + frame.bgra.len());
    out.extend_from_slice(&(frame.w as u32).to_le_bytes());
    out.extend_from_slice(&(frame.h as u32).to_le_bytes());
    for px in frame.bgra.chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
    Ok(tauri::ipc::Response::new(out))
}

fn set_locked(app: &AppHandle, shared: &Shared, locked: bool) {
    shared.locked.store(locked, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = w.set_ignore_cursor_events(locked);
    }
    let _ = app.emit("overlay-locked", locked);
}

#[tauri::command]
fn set_overlay_locked(app: AppHandle, shared: S, locked: bool) {
    set_locked(&app, &shared, locked);
}

/// Shows a sample line in the subtitle window so styles can be judged in place.
#[tauri::command]
fn overlay_demo(app: AppHandle, n: usize) {
    const SAMPLES: [(&str, &str, &str); 3] = [
        ("Race Engineer", "Box this lap, box this lap. We're going to the hard tyre.", "这圈进站，这圈进站。我们换硬胎。"),
        ("Lando Norris", "The rears are gone, I have no traction out of turn nine.", "后胎不行了，出 9 号弯完全没有抓地力。"),
        ("Race Engineer", "We need some lift and coast now, target is minus three.", "现在需要松油滑行，目标是负 3。"),
    ];
    let (speaker, text, zh) = SAMPLES[n % SAMPLES.len()];
    let _ = app.emit("overlay-demo", serde_json::json!({ "speaker": speaker, "text": text, "zh": zh }));
}

#[tauri::command]
fn set_overlay_visible(app: AppHandle, visible: bool) {
    if let Some(s) = app.try_state::<Arc<Shared>>() {
        s.overlay_hidden.store(!visible, Ordering::SeqCst);
    }
    if let Some(w) = app.get_webview_window("overlay") {
        let _ = if visible { w.show() } else { w.hide() };
        let _ = app.emit("overlay-visible", visible);
    }
}

fn show_main(app: &AppHandle, page: Option<String>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    if let Some(p) = page {
        let _ = app.emit("navigate", p);
    }
}

#[tauri::command]
fn open_main(app: AppHandle, page: Option<String>) {
    show_main(&app, page);
}

// ── region selector ──

static SELECTOR_READY: AtomicBool = AtomicBool::new(false);

// Must be async: on Windows, creating a webview inside a synchronous command
// blocks the event loop, which froze the whole desktop under the selector.
#[tauri::command]
async fn select_region(app: AppHandle) -> Result<(), String> {
    if app.get_webview_window("selector").is_some() {
        return Ok(());
    }
    let main = app.get_webview_window("main").ok_or("main window missing")?;
    let monitor = main
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| main.primary_monitor().ok().flatten())
        .ok_or("找不到显示器")?;
    let (pos, size) = (*monitor.position(), *monitor.size());
    SELECTOR_READY.store(false, Ordering::SeqCst);
    // A global Esc always gets the user out, even if the selector never takes focus.
    let _ = app.global_shortcut().register(Shortcut::new(None, Code::Escape));
    let built = WebviewWindowBuilder::new(&app, "selector", WebviewUrl::App("selector.html".into()))
        .title("框选识别区域")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build();
    let w = match built {
        Ok(w) => w,
        Err(e) => {
            let _ = app.global_shortcut().unregister(Shortcut::new(None, Code::Escape));
            return Err(format!("无法打开框选界面：{e}"));
        }
    };
    // Physical pixels: logical sizes go wrong when monitors use different scaling.
    let _ = w.set_position(PhysicalPosition::new(pos.x, pos.y));
    let _ = w.set_size(PhysicalSize::new(size.width, size.height));
    let _ = w.show();
    let _ = w.set_focus();
    // Watchdog: if the page never reports ready, take the window down again.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(8));
        if !SELECTOR_READY.load(Ordering::SeqCst) && handle.get_webview_window("selector").is_some() {
            close_selector(&handle);
        }
    });
    Ok(())
}

/// Called by the selector page once it is painted; only then are the other windows hidden.
#[tauri::command]
fn selector_ready(app: AppHandle) {
    SELECTOR_READY.store(true, Ordering::SeqCst);
    for label in ["main", "overlay"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.hide();
        }
    }
    if let Some(w) = app.get_webview_window("selector") {
        let _ = w.set_focus();
    }
}

#[derive(Serialize)]
struct SelectorInfo {
    region: Option<Region>,
}

/// The saved region in the selector window's own CSS pixels.
#[tauri::command]
fn selector_info(app: AppHandle, shared: S) -> SelectorInfo {
    let region = shared.settings.lock().unwrap().region;
    let Some(w) = app.get_webview_window("selector") else { return SelectorInfo { region: None } };
    let (Ok(origin), Ok(scale)) = (w.inner_position(), w.scale_factor()) else { return SelectorInfo { region: None } };
    let region = region.map(|r| Region {
        x: ((r.x - origin.x) as f64 / scale).round() as i32,
        y: ((r.y - origin.y) as f64 / scale).round() as i32,
        w: (r.w as f64 / scale).round() as i32,
        h: (r.h as f64 / scale).round() as i32,
    });
    SelectorInfo { region }
}

fn close_selector(app: &AppHandle) {
    let _ = app.global_shortcut().unregister(Shortcut::new(None, Code::Escape));
    if let Some(w) = app.get_webview_window("selector") {
        let _ = w.destroy();
    }
    let keep_hidden = app.try_state::<Arc<Shared>>().is_some_and(|s| s.overlay_hidden.load(Ordering::SeqCst));
    if let (Some(o), false) = (app.get_webview_window("overlay"), keep_hidden) {
        let _ = o.show();
    }
    show_main(app, Some("region".into()));
}

#[derive(Deserialize)]
struct CssRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[tauri::command]
async fn finish_region(app: AppHandle, shared: State<'_, Arc<Shared>>, rect: CssRect) -> Result<(), String> {
    let w = app.get_webview_window("selector").ok_or("selector missing")?;
    let origin = w.inner_position().map_err(|e| e.to_string())?;
    let scale = w.scale_factor().map_err(|e| e.to_string())?;
    let region = Region {
        x: origin.x + (rect.x * scale).round() as i32,
        y: origin.y + (rect.y * scale).round() as i32,
        w: (rect.w * scale).round() as i32,
        h: (rect.h * scale).round() as i32,
    };
    let snapshot = {
        let mut s = shared.settings.lock().unwrap();
        s.region = Some(region);
        s.clone()
    };
    shared.save()?;
    let _ = app.emit("settings-changed", snapshot);
    close_selector(&app);
    Ok(())
}

#[tauri::command]
async fn cancel_region(app: AppHandle) -> Result<(), String> {
    close_selector(&app);
    Ok(())
}

#[tauri::command]
fn quit(app: AppHandle, shared: S) {
    let _ = shared.save();
    app.exit(0);
}

// ── overlay placement ──

fn contains(m: &Monitor, x: i32, y: i32) -> bool {
    let (p, s) = (m.position(), m.size());
    x >= p.x && x < p.x + s.width as i32 && y >= p.y && y < p.y + s.height as i32
}

/// The monitor every window opens on: the one chosen in settings, else the primary one.
fn target_monitor(app: &AppHandle, shared: &Shared) -> Option<Monitor> {
    let main = app.get_webview_window("main")?;
    let (name, at) = {
        let s = shared.settings.lock().unwrap();
        (s.display.clone(), s.display_at)
    };
    let all = main.available_monitors().unwrap_or_default();
    // Position first: Windows can swap DISPLAY1/DISPLAY2 after a reboot.
    if let Some((x, y)) = at {
        if let Some(m) = all.iter().find(|m| m.position().x == x && m.position().y == y) {
            return Some(m.clone());
        }
    }
    if let Some(name) = name {
        if let Some(m) = all.iter().find(|m| m.name().is_some_and(|n| *n == name)) {
            return Some(m.clone());
        }
    }
    main.primary_monitor().ok().flatten()
}

fn place_main(app: &AppHandle, m: &Monitor) {
    let Some(w) = app.get_webview_window("main") else { return };
    let Ok(size) = w.outer_size() else { return };
    let (p, s) = (m.position(), m.size());
    let x = p.x + (s.width.saturating_sub(size.width) / 2) as i32;
    let y = p.y + (s.height.saturating_sub(size.height) / 2) as i32;
    let _ = w.set_position(PhysicalPosition::new(x, y));
}

fn place_overlay(app: &AppHandle, shared: &Shared) {
    let Some(w) = app.get_webview_window("overlay") else { return };
    let target = target_monitor(app, shared);
    let mut saved = shared.settings.lock().unwrap().overlay;
    // Keep a remembered spot only if it is still on the chosen monitor.
    if let (Some(r), Some(m)) = (saved, target.as_ref()) {
        if !contains(m, r.x + r.w as i32 / 2, r.y + r.h as i32 / 2) {
            saved = None;
        }
    }
    if let Some(r) = saved {
        let _ = w.set_size(PhysicalSize::new(r.w, r.h));
        let _ = w.set_position(PhysicalPosition::new(r.x, r.y));
    } else if let Some(m) = target {
        let (mp, ms, scale) = (m.position(), m.size(), m.scale_factor());
        let width = ((980.0 * scale) as u32).min(ms.width);
        let height = (260.0 * scale) as u32;
        let x = mp.x + ((ms.width - width) / 2) as i32;
        let y = mp.y + ms.height as i32 - height as i32 - (ms.height as f64 * 0.06) as i32;
        let _ = w.set_size(PhysicalSize::new(width, height));
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    let _ = w.show();
}

#[derive(Serialize)]
struct Display {
    name: String,
    label: String,
    current: bool,
}

#[tauri::command]
fn list_displays(app: AppHandle, shared: S) -> Vec<Display> {
    let Some(main) = app.get_webview_window("main") else { return vec![] };
    let primary = main.primary_monitor().ok().flatten();
    let target = target_monitor(&app, &shared).and_then(|m| m.name().cloned());
    let mut list: Vec<Monitor> = main.available_monitors().unwrap_or_default();
    list.sort_by_key(|m| (m.position().x, m.position().y));
    list.iter()
        .filter_map(|m| {
            let name = m.name()?.clone();
            let s = m.size();
            let label = match primary.as_ref() {
                Some(p) if p.name() == m.name() => "主屏幕".to_string(),
                Some(p) if m.position().x + s.width as i32 <= p.position().x => "左侧副屏".to_string(),
                Some(p) if m.position().x >= p.position().x + p.size().width as i32 => "右侧副屏".to_string(),
                _ => "副屏".to_string(),
            };
            Some(Display { current: target.as_ref() == Some(&name), label: format!("{label} {}×{}", s.width, s.height), name })
        })
        .collect()
}

/// Moves the main window and the subtitle window to the chosen monitor and remembers it.
#[tauri::command]
fn set_display(app: AppHandle, shared: S, name: String) -> Result<(), String> {
    let at = app
        .get_webview_window("main")
        .and_then(|w| w.available_monitors().ok())
        .and_then(|all| all.into_iter().find(|m| m.name().is_some_and(|n| *n == name)))
        .map(|m| (m.position().x, m.position().y));
    let snapshot = {
        let mut s = shared.settings.lock().unwrap();
        s.display = Some(name);
        s.display_at = at;
        s.overlay = None;
        s.clone()
    };
    shared.save()?;
    if let Some(m) = target_monitor(&app, &shared) {
        place_main(&app, &m);
    }
    place_overlay(&app, &shared);
    let _ = app.emit("settings-changed", snapshot);
    Ok(())
}

fn remember_overlay(app: &AppHandle) {
    let Some(w) = app.get_webview_window("overlay") else { return };
    let (Ok(p), Ok(s)) = (w.outer_position(), w.outer_size()) else { return };
    if let Some(shared) = app.try_state::<Arc<Shared>>() {
        shared.settings.lock().unwrap().overlay = Some(WindowRect { x: p.x, y: p.y, w: s.width, h: s.height });
    }
}

// ── tray & shortcuts ──

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "开始 / 暂停", true, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", "锁定 / 解锁字幕窗", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &toggle, &lock, &sep, &quit])?;
    let mut tray = TrayIconBuilder::with_id("tray")
        .tooltip("F1 Radio")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let shared = app.state::<Arc<Shared>>().inner().clone();
            match event.id().as_ref() {
                "show" => show_main(app, None),
                "toggle" => engine::toggle(app.clone(), shared),
                "lock" => set_locked(app, &shared, !shared.locked.load(Ordering::SeqCst)),
                "quit" => {
                    remember_overlay(app);
                    let _ = shared.save();
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle(), None);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn register_shortcuts(app: &AppHandle) {
    let mods = Some(Modifiers::CONTROL | Modifiers::ALT);
    for code in [Code::F9, Code::F10, Code::KeyL, Code::KeyH, Code::Comma] {
        let _ = app.global_shortcut().register(Shortcut::new(mods, code));
    }
}

fn on_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    if shortcut.key == Code::Escape {
        // Only registered while the region selector is open.
        close_selector(app);
        return;
    }
    let shared = app.state::<Arc<Shared>>().inner().clone();
    let mods = Modifiers::CONTROL | Modifiers::ALT;
    if !shortcut.mods.contains(mods) {
        return;
    }
    match shortcut.key {
        Code::F9 => engine::toggle(app.clone(), shared),
        Code::F10 => engine::stop(app, &shared),
        Code::KeyL => set_locked(app, &shared, !shared.locked.load(Ordering::SeqCst)),
        Code::KeyH => {
            if let Some(w) = app.get_webview_window("overlay") {
                let visible = w.is_visible().unwrap_or(true);
                set_overlay_visible(app.clone(), !visible);
            }
        }
        Code::Comma => show_main(app, None),
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app, None)))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        on_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // Debug builds can point at a throwaway settings file so tests never touch the real one.
            let path = match std::env::var("F1RADIO_SETTINGS") {
                Ok(p) if cfg!(debug_assertions) => std::path::PathBuf::from(p),
                _ => app.path().app_config_dir()?.join("settings.json"),
            };
            let shared = Arc::new(Shared::new(Settings::load(&path), path));
            app.manage(shared.clone());
            let handle = app.handle().clone();
            // Development aid: `F1RADIO_DRYRUN=1` prints the chosen monitor and exits before any window shows.
            if cfg!(debug_assertions) && std::env::var("F1RADIO_DRYRUN").is_ok() {
                let m = target_monitor(&handle, &shared);
                eprintln!("[dryrun] target={:?}", m.map(|m| (m.name().cloned(), *m.position(), *m.size())));
                std::process::exit(0);
            }
            build_tray(&handle)?;
            register_shortcuts(&handle);
            // Windows start hidden and appear only once they sit on the chosen monitor.
            if let Some(m) = target_monitor(&handle, &shared) {
                place_main(&handle, &m);
            }
            if let Some(w) = handle.get_webview_window("main") {
                let _ = w.show();
            }
            place_overlay(&handle, &shared);
            if cfg!(debug_assertions) {
                let pos = |l: &str| handle.get_webview_window(l).and_then(|w| w.outer_position().ok());
                eprintln!(
                    "[place] target={:?} main={:?} overlay={:?}",
                    target_monitor(&handle, &shared).map(|m| (m.name().cloned(), *m.position())),
                    pos("main"),
                    pos("overlay")
                );
            }
            // Development aid: `F1RADIO_AUTOSTART=1` starts translating right after launch.
            if cfg!(debug_assertions) && std::env::var("F1RADIO_AUTOSTART").is_ok() {
                let (h, s) = (handle.clone(), shared.clone());
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    engine::start(h, s);
                });
            }
            // Development aid: `F1RADIO_DEMO=1` plays sample lines into the subtitle window.
            if cfg!(debug_assertions) && std::env::var("F1RADIO_DEMO").is_ok() {
                let h = handle.clone();
                std::thread::spawn(move || {
                    for n in 0..3 {
                        std::thread::sleep(std::time::Duration::from_secs(if n == 0 { 7 } else { 3 }));
                        overlay_demo(h.clone(), n);
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            ("overlay", WindowEvent::Moved(_) | WindowEvent::Resized(_)) => remember_overlay(window.app_handle()),
            ("main", WindowEvent::CloseRequested { .. }) => {
                let app = window.app_handle();
                remember_overlay(app);
                if let Some(shared) = app.try_state::<Arc<Shared>>() {
                    let _ = shared.save();
                }
                app.exit(0);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            update_style,
            get_state,
            engine_cmd,
            test_provider,
            capture_preview,
            set_overlay_locked,
            set_overlay_visible,
            overlay_demo,
            list_displays,
            set_display,
            open_main,
            select_region,
            selector_ready,
            selector_info,
            finish_region,
            cancel_region,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running F1 Radio");
}
