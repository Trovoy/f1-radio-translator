mod engine;
mod settings;
#[cfg(test)]
mod tests;
mod translate;
mod vision;

use engine::{RunState, Shared};
use serde::{Deserialize, Serialize};
use settings::{Region, Settings, Style, WindowRect};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
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

#[tauri::command]
fn select_region(app: AppHandle) -> Result<(), String> {
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
    let scale = monitor.scale_factor();
    let pos = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    let _ = main.hide();
    if let Some(o) = app.get_webview_window("overlay") {
        let _ = o.hide();
    }
    WebviewWindowBuilder::new(&app, "selector", WebviewUrl::App("selector.html".into()))
        .title("框选识别区域")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .position(pos.x, pos.y)
        .inner_size(size.width, size.height)
        .focused(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
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
    if let Some(w) = app.get_webview_window("selector") {
        let _ = w.close();
    }
    if let Some(o) = app.get_webview_window("overlay") {
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
fn finish_region(app: AppHandle, shared: S, rect: CssRect) -> Result<(), String> {
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
fn cancel_region(app: AppHandle) {
    close_selector(&app);
}

#[tauri::command]
fn quit(app: AppHandle, shared: S) {
    let _ = shared.save();
    app.exit(0);
}

// ── overlay placement ──

fn place_overlay(app: &AppHandle, shared: &Shared) {
    let Some(w) = app.get_webview_window("overlay") else { return };
    let saved = shared.settings.lock().unwrap().overlay;
    if let Some(r) = saved {
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
            let path = app.path().app_config_dir()?.join("settings.json");
            let shared = Arc::new(Shared::new(Settings::load(&path), path));
            app.manage(shared.clone());
            let handle = app.handle().clone();
            build_tray(&handle)?;
            register_shortcuts(&handle);
            place_overlay(&handle, &shared);
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
            open_main,
            select_region,
            selector_info,
            finish_region,
            cancel_region,
            quit
        ])
        .run(tauri::generate_context!())
        .expect("error while running F1 Radio");
}
