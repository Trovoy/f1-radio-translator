mod engine;
mod local;
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
    // Window positions are owned by the backend; the page's copy may be stale.
    let (overlay, main_window) = {
        let s = shared.settings.lock().unwrap();
        (s.overlay, s.main_window)
    };
    let mut next = settings;
    next.overlay = overlay;
    next.main_window = main_window;
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
    overlay_visible: bool,
    ocr: String,
}

#[tauri::command]
fn get_state(shared: S) -> Snapshot {
    Snapshot {
        state: shared.run_state(),
        locked: shared.locked.load(Ordering::SeqCst),
        overlay_visible: !shared.overlay_hidden.load(Ordering::SeqCst),
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
    let job = translate::Job { text: "Box this lap, we're going to the hard tyre.", context: &[], glossary: translate::GLOSSARY, speaker: "" };
    let t0 = Instant::now();
    let text = translate::translate(&client, &id, &fields, &job).await?;
    Ok(TestResult { text, ms: t0.elapsed().as_millis() as u64 })
}

// ── built-in local models ──

#[derive(Serialize)]
struct LocalModelView {
    id: &'static str,
    name: &'static str,
    family: &'static str,
    desc: &'static str,
    size: u64,
    vram_gb: f32,
    recommended: bool,
    installed: bool,
}

#[derive(Serialize)]
struct LocalInfo {
    engine_kind: &'static str,
    engine_installed: bool,
    engine_size: u64,
    models: Vec<LocalModelView>,
    downloading: Option<String>,
    running: Option<String>,
}

#[tauri::command]
async fn local_info() -> LocalInfo {
    let l = local::get();
    LocalInfo {
        engine_kind: local::engine_kind(),
        engine_installed: l.engine_installed(),
        engine_size: local::engine_download_size(),
        models: local::MODELS
            .iter()
            .map(|m| LocalModelView {
                id: m.id,
                name: m.name,
                family: m.family,
                desc: m.desc,
                size: m.size,
                vram_gb: m.vram_gb,
                recommended: m.recommended,
                installed: l.model_installed(m),
            })
            .collect(),
        downloading: l.downloading(),
        running: l.running().await,
    }
}

#[tauri::command]
async fn local_download(app: AppHandle, id: String) -> Result<(), String> {
    local::get().download(Some(&app), &id).await
}

#[tauri::command]
fn local_cancel() {
    local::get().cancel();
}

#[tauri::command]
async fn local_delete(app: AppHandle, id: String) -> Result<(), String> {
    local::get().delete(&id).await?;
    let _ = app.emit("local-changed", ());
    Ok(())
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

// ── region selector: one window per monitor, draw on whichever holds MultiViewer ──

static SELECTOR_READY: AtomicBool = AtomicBool::new(false);

fn selector_windows(app: &AppHandle) -> Vec<tauri::WebviewWindow> {
    app.webview_windows().into_iter().filter(|(l, _)| l.starts_with("selector-")).map(|(_, w)| w).collect()
}

// Must be async: on Windows, creating a webview inside a synchronous command
// blocks the event loop, which froze the whole desktop under the selector.
#[tauri::command]
async fn select_region(app: AppHandle) -> Result<(), String> {
    if !selector_windows(&app).is_empty() {
        return Ok(());
    }
    let main = app.get_webview_window("main").ok_or("main window missing")?;
    let monitors = main.available_monitors().map_err(|e| e.to_string())?;
    if monitors.is_empty() {
        return Err("找不到显示器".into());
    }
    SELECTOR_READY.store(false, Ordering::SeqCst);
    // A global Esc always gets the user out, even if no selector takes focus.
    let _ = app.global_shortcut().register(Shortcut::new(None, Code::Escape));
    for (i, m) in monitors.iter().enumerate() {
        let built = WebviewWindowBuilder::new(&app, format!("selector-{i}"), WebviewUrl::App("selector.html".into()))
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
                close_selector(&app);
                return Err(format!("无法打开框选界面：{e}"));
            }
        };
        // Physical pixels: logical sizes go wrong when monitors use different scaling.
        let _ = w.set_position(PhysicalPosition::new(m.position().x, m.position().y));
        let _ = w.set_size(PhysicalSize::new(m.size().width, m.size().height));
        let _ = w.show();
    }
    // Watchdog: if no page reports ready, take every selector down again.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(8));
        if !SELECTOR_READY.load(Ordering::SeqCst) && !selector_windows(&handle).is_empty() {
            close_selector(&handle);
        }
    });
    Ok(())
}

/// Called by a selector page once it is painted; only then are the other windows hidden.
#[tauri::command]
fn selector_ready(app: AppHandle) {
    SELECTOR_READY.store(true, Ordering::SeqCst);
    for label in ["main", "overlay"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.hide();
        }
    }
}

#[derive(Serialize)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Serialize)]
struct SelectorInfo {
    region: Option<Region>,
    /// This monitor's top-left in physical pixels, so the page can label absolute coordinates.
    origin: Option<Point>,
}

/// The saved region in the calling selector's own CSS pixels, if it lies on that monitor.
#[tauri::command]
fn selector_info(window: tauri::WebviewWindow, shared: S) -> SelectorInfo {
    let region = shared.settings.lock().unwrap().region;
    let (Ok(origin), Ok(size), Ok(scale)) = (window.inner_position(), window.inner_size(), window.scale_factor()) else {
        return SelectorInfo { region: None, origin: None };
    };
    let region = region
        .filter(|r| {
            let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
            cx >= origin.x && cx < origin.x + size.width as i32 && cy >= origin.y && cy < origin.y + size.height as i32
        })
        .map(|r| Region {
            x: ((r.x - origin.x) as f64 / scale).round() as i32,
            y: ((r.y - origin.y) as f64 / scale).round() as i32,
            w: (r.w as f64 / scale).round() as i32,
            h: (r.h as f64 / scale).round() as i32,
        });
    SelectorInfo { region, origin: Some(Point { x: origin.x, y: origin.y }) }
}

fn close_selector(app: &AppHandle) {
    let _ = app.global_shortcut().unregister(Shortcut::new(None, Code::Escape));
    let windows = selector_windows(app);
    if windows.is_empty() {
        return;
    }
    for w in windows {
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
async fn finish_region(
    app: AppHandle,
    window: tauri::WebviewWindow,
    shared: State<'_, Arc<Shared>>,
    rect: CssRect,
) -> Result<(), String> {
    let origin = window.inner_position().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
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
    remember_overlay(&app);
    let _ = shared.save();
    local::get().stop_blocking();
    app.exit(0);
}

// ── window placement: each window reopens where it was left ──

fn contains(m: &Monitor, x: i32, y: i32) -> bool {
    let (p, s) = (m.position(), m.size());
    x >= p.x && x < p.x + s.width as i32 && y >= p.y && y < p.y + s.height as i32
}

/// A saved rect is reused only while its centre is still on some monitor (one may have been unplugged).
fn still_visible(w: &tauri::WebviewWindow, r: &WindowRect) -> bool {
    let (cx, cy) = (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2);
    w.available_monitors().unwrap_or_default().iter().any(|m| contains(m, cx, cy))
}

fn place_main(app: &AppHandle, shared: &Shared) {
    let Some(w) = app.get_webview_window("main") else { return };
    let saved = shared.settings.lock().unwrap().main_window;
    if let Some(r) = saved.filter(|r| still_visible(&w, r)) {
        let _ = w.set_size(PhysicalSize::new(r.w, r.h));
        let _ = w.set_position(PhysicalPosition::new(r.x, r.y));
    } else if let (Ok(Some(m)), Ok(size)) = (w.primary_monitor(), w.outer_size()) {
        let (p, s) = (m.position(), m.size());
        let x = p.x + (s.width.saturating_sub(size.width) / 2) as i32;
        let y = p.y + (s.height.saturating_sub(size.height) / 2) as i32;
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    let _ = w.show();
}

fn place_overlay(app: &AppHandle, shared: &Shared) {
    let Some(w) = app.get_webview_window("overlay") else { return };
    let saved = shared.settings.lock().unwrap().overlay;
    if let Some(r) = saved.filter(|r| still_visible(&w, r)) {
        let _ = w.set_size(PhysicalSize::new(r.w, r.h));
        let _ = w.set_position(PhysicalPosition::new(r.x, r.y));
    } else if let Ok(Some(m)) = w.primary_monitor() {
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

/// Keeps the latest position of the main window or the overlay in memory; it is written on save/exit.
fn remember_window(app: &AppHandle, label: &str) {
    let Some(w) = app.get_webview_window(label) else { return };
    if w.is_minimized().unwrap_or(false) || !w.is_visible().unwrap_or(true) {
        return;
    }
    let (Ok(p), Ok(s)) = (w.outer_position(), w.outer_size()) else { return };
    let rect = Some(WindowRect { x: p.x, y: p.y, w: s.width, h: s.height });
    if let Some(shared) = app.try_state::<Arc<Shared>>() {
        let mut st = shared.settings.lock().unwrap();
        if label == "main" { st.main_window = rect } else { st.overlay = rect }
    }
}

fn remember_overlay(app: &AppHandle) {
    remember_window(app, "overlay");
    remember_window(app, "main");
}

// ── tray & shortcuts ──

/// Opens the styled tray menu window with its bottom-right corner at the cursor, kept on-screen.
fn show_tray_menu(app: &AppHandle, at: PhysicalPosition<f64>) {
    let Some(w) = app.get_webview_window("traymenu") else { return };
    let Ok(size) = w.outer_size() else { return };
    let (cx, cy) = (at.x as i32, at.y as i32);
    let mut x = cx - size.width as i32;
    let mut y = cy - size.height as i32;
    if let Ok(Some(m)) = w.monitor_from_point(at.x, at.y) {
        let (p, s) = (m.position(), m.size());
        x = x.clamp(p.x, p.x + s.width as i32 - size.width as i32);
        y = y.clamp(p.y, p.y + s.height as i32 - size.height as i32);
    }
    let _ = w.set_position(PhysicalPosition::new(x, y));
    let _ = w.show();
    let _ = w.set_focus();
    let _ = app.emit_to("traymenu", "traymenu-open", ());
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let mut tray = TrayIconBuilder::with_id("tray")
        .tooltip("F1 Radio")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } => {
                show_main(tray.app_handle(), None)
            }
            TrayIconEvent::Click { button: MouseButton::Right, button_state: MouseButtonState::Up, position, .. } => {
                show_tray_menu(tray.app_handle(), position)
            }
            _ => {}
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
            local::init(app.path().app_local_data_dir()?.join("local"));
            let handle = app.handle().clone();
            // Development aid: `F1RADIO_DRYRUN=1` prints where windows would open and exits before any shows.
            if cfg!(debug_assertions) && std::env::var("F1RADIO_DRYRUN").is_ok() {
                let s = shared.settings.lock().unwrap();
                eprintln!("[dryrun] main={:?} overlay={:?}", s.main_window, s.overlay);
                std::process::exit(0);
            }
            // Windows are created only now, after the state exists, so no page can call a command too early.
            for cfg in app.config().app.windows.clone() {
                WebviewWindowBuilder::from_config(&handle, &cfg)?.build()?;
            }
            build_tray(&handle)?;
            register_shortcuts(&handle);
            // Windows start hidden and appear only once they are back where they were left.
            place_main(&handle, &shared);
            place_overlay(&handle, &shared);
            if cfg!(debug_assertions) {
                let pos = |l: &str| handle.get_webview_window(l).and_then(|w| w.outer_position().ok());
                eprintln!("[place] main={:?} overlay={:?}", pos("main"), pos("overlay"));
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
            ("overlay" | "main", WindowEvent::Moved(_) | WindowEvent::Resized(_)) => remember_window(window.app_handle(), window.label()),
            ("main", WindowEvent::CloseRequested { .. }) => {
                let app = window.app_handle();
                remember_overlay(app);
                if let Some(shared) = app.try_state::<Arc<Shared>>() {
                    let _ = shared.save();
                }
                local::get().stop_blocking();
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
            local_info,
            local_download,
            local_cancel,
            local_delete,
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
