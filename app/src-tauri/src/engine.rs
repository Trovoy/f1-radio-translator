//! The live loop: a dedicated thread captures the region and runs OCR,
//! new cards go through a channel to async translation workers, and the
//! results reach both windows as events.

use crate::settings::Settings;
use crate::translate::{self, Job, GLOSSARY};
use crate::vision::{self, Ocr};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Semaphore};

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    Idle,
    Live,
    Paused,
}

pub struct Shared {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    pub state: Mutex<RunState>,
    /// Bumped on every start and stop so a stale loop knows to exit.
    pub generation: AtomicU64,
    pub locked: AtomicBool,
    pub overlay_hidden: AtomicBool,
    pub ocr_language: Mutex<String>,
    cooldown: Mutex<HashMap<String, Instant>>,
    history: Mutex<HashMap<String, VecDeque<String>>>,
    next_id: AtomicU64,
}

impl Shared {
    pub fn new(settings: Settings, settings_path: PathBuf) -> Self {
        Self {
            settings: Mutex::new(settings),
            settings_path,
            state: Mutex::new(RunState::Idle),
            generation: AtomicU64::new(0),
            locked: AtomicBool::new(false),
            overlay_hidden: AtomicBool::new(false),
            ocr_language: Mutex::new(String::new()),
            cooldown: Mutex::new(HashMap::new()),
            history: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn run_state(&self) -> RunState {
        *self.state.lock().unwrap()
    }

    pub fn save(&self) -> Result<(), String> {
        self.settings.lock().unwrap().save(&self.settings_path)
    }
}

#[derive(Clone, Serialize)]
pub struct Status {
    pub state: RunState,
    pub level: &'static str,
    pub message: String,
}

#[derive(Clone, Serialize)]
struct Line {
    id: u64,
    speaker: String,
    text: String,
}

#[derive(Clone, Serialize)]
struct Translated {
    id: u64,
    zh: String,
    provider: String,
    ms: u64,
}

#[derive(Clone, Serialize)]
struct Failed {
    id: u64,
    error: String,
}

pub fn status(app: &AppHandle, shared: &Shared, level: &'static str, message: impl Into<String>) {
    let _ = app.emit("radio-status", Status { state: shared.run_state(), level, message: message.into() });
}

pub fn start(app: AppHandle, shared: Arc<Shared>) {
    {
        let mut st = shared.state.lock().unwrap();
        match *st {
            RunState::Live => return,
            RunState::Paused => {
                *st = RunState::Live;
                drop(st);
                status(&app, &shared, "info", "已恢复识别");
                return;
            }
            RunState::Idle => *st = RunState::Live,
        }
    }
    if shared.settings.lock().unwrap().region.is_none() {
        *shared.state.lock().unwrap() = RunState::Idle;
        status(&app, &shared, "error", "请先框选 MultiViewer 的无线电转录区域");
        return;
    }
    let gen = shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
    shared.history.lock().unwrap().clear();
    let (tx, rx) = mpsc::channel::<Line>(64);
    status(&app, &shared, "info", "正在启动文字识别…");

    let (a, s) = (app.clone(), shared.clone());
    std::thread::spawn(move || scan_loop(a, s, gen, tx));
    let (a, s) = (app.clone(), shared.clone());
    tauri::async_runtime::spawn(async move { translate_loop(a, s, gen, rx).await });
}

pub fn pause(app: &AppHandle, shared: &Shared) {
    let mut st = shared.state.lock().unwrap();
    if *st == RunState::Live {
        *st = RunState::Paused;
        drop(st);
        status(app, shared, "info", "已暂停");
    }
}

pub fn stop(app: &AppHandle, shared: &Shared) {
    shared.generation.fetch_add(1, Ordering::SeqCst);
    *shared.state.lock().unwrap() = RunState::Idle;
    status(app, shared, "info", "已结束");
}

pub fn toggle(app: AppHandle, shared: Arc<Shared>) {
    match shared.run_state() {
        RunState::Live => pause(&app, &shared),
        _ => start(app, shared),
    }
}

fn scan_loop(app: AppHandle, shared: Arc<Shared>, gen: u64, tx: mpsc::Sender<Line>) {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    let alive = || shared.generation.load(Ordering::SeqCst) == gen;
    let ocr = match Ocr::new() {
        Ok(o) => o,
        Err(e) => {
            if alive() {
                *shared.state.lock().unwrap() = RunState::Idle;
                status(&app, &shared, "error", e);
            }
            return;
        }
    };
    *shared.ocr_language.lock().unwrap() = ocr.language.clone();
    status(&app, &shared, "info", format!("正在识别 · {}", ocr.language));

    let mut last_fp = 0u64;
    let mut seen_order: VecDeque<String> = VecDeque::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut first = true;
    let mut warned = false;

    while alive() {
        let (region, interval, skip) = {
            let s = shared.settings.lock().unwrap();
            (s.region, s.interval_ms.clamp(400, 5000), s.skip_unchanged)
        };
        if shared.run_state() != RunState::Live {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        let Some(region) = region else {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        };
        match vision::capture(&region) {
            Ok(frame) => {
                let fp = frame.fingerprint();
                if !(skip && fp == last_fp) {
                    last_fp = fp;
                    match ocr.recognize(&frame.upscale2()) {
                        Ok(rows) => {
                            warned = false;
                            let cards: Vec<_> = vision::extract_cards(rows)
                                .into_iter()
                                .filter(|c| vision::is_transcript(&c.text))
                                .collect();
                            // On the first pass only the newest few cards are translated,
                            // so starting mid-session does not flood the queue.
                            let skip_n = if first { cards.len().saturating_sub(3) } else { 0 };
                            for (i, card) in cards.into_iter().enumerate() {
                                let key = format!("{}\n{}", card.speaker, card.text);
                                if !seen.insert(key.clone()) {
                                    continue;
                                }
                                seen_order.push_back(key);
                                if seen_order.len() > 400 {
                                    if let Some(old) = seen_order.pop_front() {
                                        seen.remove(&old);
                                    }
                                }
                                if i < skip_n {
                                    continue;
                                }
                                let line = Line {
                                    id: shared.next_id.fetch_add(1, Ordering::SeqCst),
                                    speaker: card.speaker,
                                    text: card.text,
                                };
                                let _ = app.emit("radio-line", line.clone());
                                if tx.blocking_send(line).is_err() {
                                    return;
                                }
                            }
                            first = false;
                        }
                        Err(e) => {
                            if !warned {
                                status(&app, &shared, "warn", e);
                                warned = true;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                if !warned {
                    status(&app, &shared, "warn", e);
                    warned = true;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(interval));
    }
}

async fn translate_loop(app: AppHandle, shared: Arc<Shared>, gen: u64, mut rx: mpsc::Receiver<Line>) {
    let client = translate::client();
    let gate = Arc::new(Semaphore::new(2));
    while let Some(line) = rx.recv().await {
        if shared.generation.load(Ordering::SeqCst) != gen {
            break;
        }
        let Ok(permit) = gate.clone().acquire_owned().await else { break };
        let (app, shared, client) = (app.clone(), shared.clone(), client.clone());
        tauri::async_runtime::spawn(async move {
            let _permit = permit;
            let result = translate_with_chain(&client, &shared, &line.speaker, &line.text).await;
            if shared.generation.load(Ordering::SeqCst) != gen {
                return;
            }
            match result {
                Ok((zh, provider, ms)) => {
                    let _ = app.emit("radio-translated", Translated { id: line.id, zh, provider, ms });
                }
                Err(error) => {
                    status(&app, &shared, "warn", format!("翻译失败：{error}"));
                    let _ = app.emit("radio-failed", Failed { id: line.id, error });
                }
            }
        });
    }
}

/// Tries each provider in order. A provider that answers 429 sits out for a minute.
pub async fn translate_with_chain(
    client: &reqwest::Client,
    shared: &Shared,
    speaker: &str,
    text: &str,
) -> Result<(String, String, u64), String> {
    let (chain, providers, fallback, ctx_n) = {
        let s = shared.settings.lock().unwrap();
        (s.chain.clone(), s.providers.clone(), s.auto_fallback, s.context_lines as usize)
    };
    let context: Vec<String> = {
        let h = shared.history.lock().unwrap();
        h.get(speaker).map(|q| q.iter().rev().take(ctx_n).rev().cloned().collect()).unwrap_or_default()
    };
    let candidates: Vec<String> = if fallback { chain } else { chain.into_iter().take(1).collect() };
    let mut last_err = "没有可用的翻译服务".to_string();
    let empty = HashMap::new();
    for (i, id) in candidates.iter().enumerate() {
        let cooling = shared.cooldown.lock().unwrap().get(id).is_some_and(|t| t.elapsed() < Duration::from_secs(60));
        if cooling && i + 1 < candidates.len() {
            continue;
        }
        let cfg = providers.get(id).unwrap_or(&empty);
        let job = Job { text, context: &context, glossary: GLOSSARY };
        let t0 = Instant::now();
        match translate::translate(client, id, cfg, &job).await {
            Ok(zh) => {
                let mut h = shared.history.lock().unwrap();
                let q = h.entry(speaker.to_string()).or_default();
                q.push_back(text.to_string());
                while q.len() > 6 {
                    q.pop_front();
                }
                return Ok((zh, id.clone(), t0.elapsed().as_millis() as u64));
            }
            Err(e) => {
                if e.contains("429") || e.to_ascii_lowercase().contains("too many") {
                    shared.cooldown.lock().unwrap().insert(id.clone(), Instant::now());
                }
                last_err = e;
            }
        }
    }
    Err(last_err)
}
