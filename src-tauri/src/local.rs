//! Built-in local translation: llama.cpp's `llama-server` runs a GGUF model on the GPU.
//! The engine and models are downloaded on first use into `%LOCALAPPDATA%\com.trovoy.f1radio\local`,
//! or a folder the user picks (e.g. on another drive), so the installer stays small.

use futures_util::StreamExt;
use serde::Serialize;
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// Tencent HY-MT: a single user message in its own translation template.
    HyMt,
    /// General chat model: the full system prompt with glossary and context.
    Chat,
}

pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub family: &'static str,
    pub desc: &'static str,
    pub file: &'static str,
    pub size: u64,
    pub vram_gb: f32,
    pub ms_repo: &'static str,
    pub hf_repo: &'static str,
    pub style: Style,
    pub recommended: bool,
}

pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "hy-mt-7b",
        name: "混元翻译 HY-MT1.5 7B",
        family: "翻译专用",
        desc: "腾讯官方翻译模型，逐句直译，F1 术语和车手名会作为参考译法传给它。碎片化的无线电短句偶尔会译得生硬。",
        file: "HY-MT1.5-7B-Q4_K_M.gguf",
        size: 4_624_649_312,
        vram_gb: 6.0,
        ms_repo: "Tencent-Hunyuan/HY-MT1.5-7B-GGUF",
        hf_repo: "tencent/HY-MT1.5-7B-GGUF",
        style: Style::HyMt,
        recommended: false,
    },
    ModelInfo {
        id: "hy-mt-1.8b",
        name: "混元翻译 HY-MT1.5 1.8B",
        family: "翻译专用",
        desc: "同系列的轻量版，体积小、速度最快，显存占用很低。",
        file: "HY-MT1.5-1.8B-Q4_K_M.gguf",
        size: 1_133_080_512,
        vram_gb: 2.0,
        ms_repo: "Tencent-Hunyuan/HY-MT1.5-1.8B-GGUF",
        hf_repo: "tencent/HY-MT1.5-1.8B-GGUF",
        style: Style::HyMt,
        recommended: false,
    },
    ModelInfo {
        id: "qwen3-8b",
        name: "通义千问 Qwen3 8B",
        family: "通用大模型",
        desc: "懂 F1 语境，完整遵守术语表、上下文和说话人信息，译文简洁口语化，最贴近真实无线电。实测每句约 0.3 秒。",
        file: "Qwen3-8B-Q4_K_M.gguf",
        size: 5_027_783_488,
        vram_gb: 6.5,
        ms_repo: "Qwen/Qwen3-8B-GGUF",
        hf_repo: "Qwen/Qwen3-8B-GGUF",
        style: Style::Chat,
        recommended: true,
    },
    ModelInfo {
        id: "qwen3-14b",
        name: "通义千问 Qwen3 14B",
        family: "通用大模型",
        desc: "同系列的大尺寸版本，质量最高，速度稍慢，需要约 10 GB 显存。",
        file: "Qwen3-14B-Q4_K_M.gguf",
        size: 9_001_752_960,
        vram_gb: 10.0,
        ms_repo: "Qwen/Qwen3-14B-GGUF",
        hf_repo: "Qwen/Qwen3-14B-GGUF",
        style: Style::Chat,
        recommended: false,
    },
];

pub fn model(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

/// llama.cpp release the engine is pinned to; tested with the models above.
const BUILD: &str = "b12211";

struct Package {
    url: String,
    size: u64,
}

/// CUDA for NVIDIA cards (fastest), Vulkan for every other GPU.
pub fn engine_kind() -> &'static str {
    let nvidia = std::env::var("WINDIR").map(|w| Path::new(&w).join("System32").join("nvcuda.dll").exists()).unwrap_or(false);
    if nvidia { "cuda" } else { "vulkan" }
}

fn packages(kind: &str) -> Vec<Package> {
    let base = format!("https://github.com/ggml-org/llama.cpp/releases/download/{BUILD}");
    if kind == "cuda" {
        vec![
            Package { url: format!("{base}/llama-{BUILD}-bin-win-cuda-12.4-x64.zip"), size: 288_918_486 },
            Package { url: format!("{base}/cudart-llama-bin-win-cuda-12.4-x64.zip"), size: 391_443_627 },
        ]
    } else {
        vec![Package { url: format!("{base}/llama-{BUILD}-bin-win-vulkan-x64.zip"), size: 37_845_757 }]
    }
}

pub fn engine_download_size() -> u64 {
    packages(engine_kind()).iter().map(|p| p.size).sum()
}

struct Server {
    child: Child,
    port: u16,
    model: String,
}

pub struct Local {
    root: Mutex<PathBuf>,
    default_root: PathBuf,
    server: tokio::sync::Mutex<Option<Server>>,
    downloading: Mutex<Option<String>>,
    cancel: AtomicBool,
    stats: Mutex<StatsInner>,
}

static LOCAL: OnceLock<Local> = OnceLock::new();

/// `chosen` is the folder from the settings; without one the app's own data folder is used.
pub fn init(default_root: PathBuf, chosen: Option<PathBuf>) {
    let _ = LOCAL.set(Local {
        root: Mutex::new(chosen.unwrap_or_else(|| default_root.clone())),
        default_root,
        server: tokio::sync::Mutex::new(None),
        downloading: Mutex::new(None),
        cancel: AtomicBool::new(false),
        stats: Mutex::new(StatsInner::default()),
    });
}

pub fn get() -> &'static Local {
    LOCAL.get().expect("local engine not initialised")
}

static APP: OnceLock<AppHandle> = OnceLock::new();

/// Lets the engine push load and speed updates to the pages.
pub fn attach(app: AppHandle) {
    let _ = APP.set(app);
}

fn notify(event: &str, payload: impl Serialize + Clone) {
    if let Some(a) = APP.get() {
        let _ = a.emit(event, payload);
    }
}

struct Sample {
    ms: u64,
    prompt_n: f64,
    prompt_ms: f64,
    gen_n: f64,
    gen_ms: f64,
}

#[derive(Default)]
struct StatsInner {
    model: String,
    load_ms: u64,
    calls: u64,
    tokens: u64,
    samples: VecDeque<Sample>,
}

/// Rolling speed numbers for the loaded model, over its last 30 lines.
#[derive(Clone, Default, Serialize)]
pub struct Stats {
    pub model: String,
    pub load_ms: u64,
    pub calls: u64,
    pub tokens: u64,
    pub last_ms: u64,
    pub avg_ms: u64,
    /// Generated tokens per second.
    pub gen_tps: f64,
    /// Prompt tokens per second (the part before the first output token).
    pub prompt_tps: f64,
    /// Average time to the first output token.
    pub first_ms: u64,
    pub recent: Vec<u64>,
}

#[derive(Clone, Serialize)]
pub struct Gpu {
    pub used_mb: u64,
    pub total_mb: u64,
    pub util: u64,
}

/// NVIDIA cards only: memory and load from nvidia-smi, which ships with the driver.
pub fn gpu() -> Option<Gpu> {
    if engine_kind() != "cuda" {
        return None;
    }
    let mut cmd = Command::new("nvidia-smi");
    cmd.args(["--query-gpu=memory.used,memory.total,utilization.gpu", "--format=csv,noheader,nounits"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let v: Vec<u64> = text.lines().next()?.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    (v.len() == 3).then(|| Gpu { used_mb: v[0], total_mb: v[1], util: v[2] })
}

#[derive(Clone, Serialize)]
struct Progress {
    task: &'static str,
    model: String,
    label: String,
    done: u64,
    total: u64,
    speed: f64,
}

impl Local {
    pub fn root(&self) -> PathBuf {
        self.root.lock().unwrap().clone()
    }

    pub fn default_root(&self) -> &Path {
        &self.default_root
    }

    fn engine_dir(&self) -> PathBuf {
        self.root().join("engine").join(format!("{BUILD}-{}", engine_kind()))
    }

    fn model_path(&self, m: &ModelInfo) -> PathBuf {
        self.root().join("models").join(m.file)
    }

    pub fn server_exe(&self) -> Option<PathBuf> {
        find_file(&self.engine_dir(), "llama-server.exe", 3)
    }

    pub fn engine_installed(&self) -> bool {
        self.server_exe().is_some()
    }

    pub fn model_installed(&self, m: &ModelInfo) -> bool {
        fs::metadata(self.model_path(m)).map(|md| md.len() == m.size).unwrap_or(false)
    }

    pub fn downloading(&self) -> Option<String> {
        self.downloading.lock().unwrap().clone()
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    pub async fn running(&self) -> Option<String> {
        let mut s = self.server.lock().await;
        let alive = s.as_mut().is_some_and(|sv| matches!(sv.child.try_wait(), Ok(None)));
        if !alive {
            *s = None;
        }
        s.as_ref().map(|sv| sv.model.clone())
    }

    /// Downloads the engine (first time only) and the model, reporting progress as events.
    pub async fn download(&self, app: Option<&AppHandle>, id: &str) -> Result<(), String> {
        let m = model(id).ok_or("未知模型")?;
        {
            let mut d = self.downloading.lock().unwrap();
            if d.is_some() {
                return Err("已有下载在进行中".into());
            }
            *d = Some(id.to_string());
        }
        self.cancel.store(false, Ordering::SeqCst);
        let result = self.download_inner(app, m).await;
        *self.downloading.lock().unwrap() = None;
        if let Some(a) = app {
            let _ = a.emit("local-changed", ());
        }
        result
    }

    async fn download_inner(&self, app: Option<&AppHandle>, m: &ModelInfo) -> Result<(), String> {
        let client = reqwest::Client::builder()
            .user_agent("F1Radio/2.0")
            .connect_timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| e.to_string())?;
        if !self.engine_installed() {
            let dl = self.root().join("downloads");
            fs::create_dir_all(&dl).map_err(|e| e.to_string())?;
            let pkgs = packages(engine_kind());
            let total: u64 = pkgs.iter().map(|p| p.size).sum();
            let mut base = 0u64;
            let mut zips = Vec::new();
            for p in &pkgs {
                let name = p.url.rsplit('/').next().unwrap_or("engine.zip");
                let dest = dl.join(name);
                fetch(self, app, &client, &[p.url.clone()], &dest, p.size, base, total, "engine", m.id, "推理引擎").await?;
                base += p.size;
                zips.push(dest);
            }
            let dir = self.engine_dir();
            emit(app, Progress { task: "unpack", model: m.id.into(), label: "正在解压推理引擎".into(), done: 0, total: 0, speed: 0.0 });
            tokio::task::spawn_blocking(move || -> Result<(), String> {
                fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                for z in &zips {
                    let f = File::open(z).map_err(|e| e.to_string())?;
                    zip::ZipArchive::new(f).and_then(|mut a| a.extract(&dir)).map_err(|e| format!("解压失败：{e}"))?;
                    let _ = fs::remove_file(z);
                }
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())??;
            if !self.engine_installed() {
                return Err("推理引擎解压后找不到 llama-server.exe".into());
            }
        }
        if !self.model_installed(m) {
            let dest = self.model_path(m);
            fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
            let urls = [
                format!("https://modelscope.cn/models/{}/resolve/master/{}", m.ms_repo, m.file),
                format!("https://huggingface.co/{}/resolve/main/{}", m.hf_repo, m.file),
            ];
            fetch(self, app, &client, &urls, &dest, m.size, 0, m.size, "model", m.id, m.name).await?;
        }
        Ok(())
    }

    /// Moves the engine and models to `target` and uses it from now on.
    /// Same-drive moves are instant renames; across drives the files are copied with progress.
    pub async fn relocate(&self, target: PathBuf) -> Result<PathBuf, String> {
        // A bare drive such as `D:\` gets its own folder instead of filling the drive root.
        let target = if target.parent().is_none() { target.join("F1 Radio") } else { target };
        let current = self.root();
        if same_path(&current, &target) {
            return Ok(current);
        }
        if target.starts_with(&current) || current.starts_with(&target) {
            return Err("新位置不能在当前位置里面，也不能包含当前位置".into());
        }
        {
            let mut d = self.downloading.lock().unwrap();
            if d.is_some() {
                return Err("下载进行中，完成或取消后再更改位置".into());
            }
            *d = Some("__move".into());
        }
        self.stop().await;
        let (from, to) = (current.clone(), target.clone());
        let result = tokio::task::spawn_blocking(move || move_tree(&from, &to)).await.map_err(|e| e.to_string()).and_then(|r| r);
        *self.downloading.lock().unwrap() = None;
        result?;
        *self.root.lock().unwrap() = target.clone();
        notify("local-changed", ());
        Ok(target)
    }

    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let m = model(id).ok_or("未知模型")?;
        if self.running().await.as_deref() == Some(id) {
            self.stop().await;
        }
        let p = self.model_path(m);
        for f in [p.clone(), p.with_extension("gguf.part")] {
            if f.exists() {
                fs::remove_file(&f).map_err(|e| format!("删除失败：{e}"))?;
            }
        }
        Ok(())
    }

    /// Starts `llama-server` with the model on the GPU (or reuses it) and waits until it is ready.
    pub async fn ensure_server(&self, id: &str) -> Result<u16, String> {
        let m = model(id).ok_or("请先在「本地模型」里选择一个模型")?;
        let mut guard = self.server.lock().await;
        if let Some(sv) = guard.as_mut() {
            if sv.model == id && matches!(sv.child.try_wait(), Ok(None)) {
                return Ok(sv.port);
            }
            let _ = sv.child.kill();
            let _ = sv.child.wait();
            *guard = None;
        }
        let exe = self.server_exe().ok_or("推理引擎尚未下载")?;
        if !self.model_installed(m) {
            return Err(format!("{} 尚未下载", m.name));
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).map_err(|e| e.to_string())?;
        let log_path = self.root().join("server.log");
        let log = File::create(&log_path).map_err(|e| e.to_string())?;
        let log2 = log.try_clone().map_err(|e| e.to_string())?;
        let mut cmd = Command::new(&exe);
        cmd.arg("-m")
            .arg(self.model_path(m))
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            // All layers on the GPU, two parallel slots (the engine translates two lines at once).
            .args(["-ngl", "99", "-c", "8192", "-np", "2", "--jinja", "--no-webui"])
            .current_dir(exe.parent().unwrap())
            .stdin(Stdio::null())
            .stdout(log)
            .stderr(log2);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd.spawn().map_err(|e| format!("无法启动推理引擎：{e}"))?;
        tie_to_app(&child);

        let health = format!("http://127.0.0.1:{port}/health");
        let client = reqwest::Client::new();
        let t0 = Instant::now();
        loop {
            if let Ok(Some(status)) = child.try_wait() {
                let tail = fs::read_to_string(&log_path).unwrap_or_default();
                let tail: String = tail.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
                return Err(format!("推理引擎退出（{status}）：{tail}"));
            }
            if let Ok(r) = client.get(&health).timeout(Duration::from_secs(2)).send().await {
                if r.status().is_success() {
                    break;
                }
            }
            if t0.elapsed() > Duration::from_secs(180) {
                let _ = child.kill();
                return Err("本地模型加载超时".into());
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        *guard = Some(Server { child, port, model: id.to_string() });
        drop(guard);
        self.record_load(id, t0.elapsed().as_millis() as u64);
        notify("local-changed", ());
        Ok(port)
    }

    fn record_load(&self, model: &str, ms: u64) {
        {
            let mut s = self.stats.lock().unwrap();
            if s.model != model {
                *s = StatsInner { model: model.into(), ..Default::default() };
            }
            s.load_ms = ms;
        }
        notify("local-stats", self.stats());
    }

    /// Called after every translation with llama.cpp's own `timings` block.
    pub fn record_call(&self, model: &str, ms: u64, timings: Option<&serde_json::Value>) {
        let num = |k: &str| timings.and_then(|t| t.get(k)).and_then(|v| v.as_f64()).unwrap_or(0.0);
        {
            let mut s = self.stats.lock().unwrap();
            if s.model != model {
                *s = StatsInner { model: model.into(), ..Default::default() };
            }
            let sample = Sample { ms, prompt_n: num("prompt_n"), prompt_ms: num("prompt_ms"), gen_n: num("predicted_n"), gen_ms: num("predicted_ms") };
            s.calls += 1;
            s.tokens += sample.gen_n as u64;
            s.samples.push_back(sample);
            while s.samples.len() > 30 {
                s.samples.pop_front();
            }
        }
        notify("local-stats", self.stats());
    }

    pub fn stats(&self) -> Stats {
        let s = self.stats.lock().unwrap();
        let n = s.samples.len().max(1) as f64;
        let sum = |f: fn(&Sample) -> f64| s.samples.iter().map(f).sum::<f64>();
        let rate = |tok: f64, ms: f64| if ms > 0.0 { tok / ms * 1000.0 } else { 0.0 };
        Stats {
            model: s.model.clone(),
            load_ms: s.load_ms,
            calls: s.calls,
            tokens: s.tokens,
            last_ms: s.samples.back().map_or(0, |x| x.ms),
            avg_ms: (sum(|x| x.ms as f64) / n) as u64,
            gen_tps: rate(sum(|x| x.gen_n), sum(|x| x.gen_ms)),
            prompt_tps: rate(sum(|x| x.prompt_n), sum(|x| x.prompt_ms)),
            first_ms: (sum(|x| x.prompt_ms) / n) as u64,
            recent: s.samples.iter().map(|x| x.ms).collect(),
        }
    }

    /// Stops the server and frees the VRAM.
    pub async fn stop(&self) {
        let mut guard = self.server.lock().await;
        if let Some(mut sv) = guard.take() {
            let _ = sv.child.kill();
            let _ = sv.child.wait();
            drop(guard);
            notify("local-changed", ());
        }
    }

    /// Synchronous stop for the exit path.
    pub fn stop_blocking(&self) {
        if let Ok(mut guard) = self.server.try_lock() {
            if let Some(mut sv) = guard.take() {
                let _ = sv.child.kill();
            }
        }
    }
}

fn emit(app: Option<&AppHandle>, p: Progress) {
    if let Some(a) = app {
        let _ = a.emit("local-progress", p);
    } else if p.task != "unpack" && p.total > 0 && (p.done == p.total || p.speed > 0.0) {
        // Tests run without a window; a line per second or so is enough there.
        static LAST: Mutex<Option<Instant>> = Mutex::new(None);
        let mut last = LAST.lock().unwrap();
        if p.done == p.total || last.map_or(true, |t| t.elapsed() > Duration::from_secs(5)) {
            *last = Some(Instant::now());
            eprintln!("[download] {} {:.1}% {:.1} MB/s", p.label, p.done as f64 / p.total as f64 * 100.0, p.speed / 1048576.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn fetch(
    local: &Local,
    app: Option<&AppHandle>,
    client: &reqwest::Client,
    urls: &[String],
    dest: &Path,
    size: u64,
    base: u64,
    total: u64,
    task: &'static str,
    model_id: &str,
    label: &str,
) -> Result<(), String> {
    let part = dest.with_extension(format!("{}.part", dest.extension().and_then(|e| e.to_str()).unwrap_or("bin")));
    let mut last_err = String::from("下载失败");
    for url in urls {
        let mut have = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if have > size {
            let _ = fs::remove_file(&part);
            have = 0;
        }
        if have == size && size > 0 {
            fs::rename(&part, dest).map_err(|e| e.to_string())?;
            return Ok(());
        }
        let mut req = client.get(url);
        if have > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={have}-"));
        }
        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                last_err = format!("连接失败：{e}");
                continue;
            }
        };
        let status = resp.status();
        if !status.is_success() {
            last_err = format!("HTTP {}", status.as_u16());
            continue;
        }
        // A server that ignores Range sends the whole file again.
        if have > 0 && status.as_u16() != 206 {
            have = 0;
        }
        let mut file = match OpenOptions::new().create(true).write(true).append(have > 0).truncate(have == 0).open(&part) {
            Ok(f) => f,
            Err(e) => return Err(format!("无法写入文件：{e}")),
        };
        let started = Instant::now();
        let start_have = have;
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        let mut stream = resp.bytes_stream();
        let mut broke = false;
        while let Some(chunk) = stream.next().await {
            if local.cancel.load(Ordering::SeqCst) {
                return Err("已取消".into());
            }
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    last_err = format!("下载中断：{e}");
                    broke = true;
                    break;
                }
            };
            file.write_all(&chunk).map_err(|e| format!("无法写入文件：{e}"))?;
            have += chunk.len() as u64;
            if last_emit.elapsed() > Duration::from_millis(250) {
                last_emit = Instant::now();
                let secs = started.elapsed().as_secs_f64().max(0.001);
                emit(app, Progress { task, model: model_id.into(), label: label.into(), done: base + have, total, speed: (have - start_have) as f64 / secs });
            }
        }
        drop(file);
        if !broke && have == size {
            fs::rename(&part, dest).map_err(|e| e.to_string())?;
            emit(app, Progress { task, model: model_id.into(), label: label.into(), done: base + size, total, speed: 0.0 });
            return Ok(());
        }
        if !broke {
            last_err = format!("文件大小不符（{have} / {size}）");
        }
    }
    Err(last_err)
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    norm(a) == norm(b)
}

/// Every file under `dir`, with its size.
fn walk(dir: &Path, out: &mut Vec<(PathBuf, u64)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        match e.metadata() {
            Ok(m) if m.is_dir() => walk(&p, out),
            Ok(m) => out.push((p, m.len())),
            Err(_) => {}
        }
    }
}

/// Moves `engine`, `models` and unfinished `downloads` from one root to another, then tidies the old root.
pub(crate) fn move_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("无法创建文件夹：{e}"))?;
    let probe = to.join(".write-test");
    fs::write(&probe, b"").map_err(|e| format!("这个位置不能写入：{e}"))?;
    let _ = fs::remove_file(&probe);

    let mut files = Vec::new();
    for sub in ["engine", "models", "downloads"] {
        walk(&from.join(sub), &mut files);
    }
    let total: u64 = files.iter().map(|(_, n)| n).sum();
    let mut done = 0u64;
    let started = Instant::now();
    let mut last = Instant::now() - Duration::from_secs(1);
    for (src, len) in &files {
        let rel = src.strip_prefix(from).map_err(|e| e.to_string())?;
        let dst = to.join(rel);
        if let Some(dir) = dst.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("无法创建文件夹：{e}"))?;
        }
        if fs::metadata(&dst).is_ok_and(|m| m.len() == *len) {
            // Already there (an earlier move was interrupted): keep the copy at the target.
            let _ = fs::remove_file(src);
        } else if fs::rename(src, &dst).is_err() {
            // Different drive: copy in chunks so the page can show progress.
            let mut r = File::open(src).map_err(|e| e.to_string())?;
            let part = dst.with_extension("moving");
            let mut w = File::create(&part).map_err(|e| format!("写入失败：{e}"))?;
            let mut buf = vec![0u8; 4 << 20];
            loop {
                let n = std::io::Read::read(&mut r, &mut buf).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                w.write_all(&buf[..n]).map_err(|e| format!("写入失败（磁盘空间够吗？）：{e}"))?;
                done += n as u64;
                if last.elapsed() > Duration::from_millis(250) {
                    last = Instant::now();
                    let secs = started.elapsed().as_secs_f64().max(0.001);
                    notify("local-progress", Progress { task: "move", model: "__move".into(), label: "正在移动".into(), done, total, speed: done as f64 / secs });
                }
            }
            w.sync_all().map_err(|e| e.to_string())?;
            drop(w);
            fs::rename(&part, &dst).map_err(|e| e.to_string())?;
            fs::remove_file(src).map_err(|e| format!("已复制，但无法删除原文件：{e}"))?;
            continue;
        }
        done += len;
    }
    for sub in ["engine", "models", "downloads"] {
        let _ = fs::remove_dir_all(from.join(sub));
    }
    let _ = fs::remove_file(from.join("server.log"));
    let _ = fs::remove_dir(from);
    Ok(())
}

/// Free space on the drive holding `path` (or its nearest existing parent).
pub fn free_bytes(path: &Path) -> Option<u64> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    let mut free = 0u64;
    unsafe { GetDiskFreeSpaceExW(&HSTRING::from(p.as_os_str()), Some(&mut free), None, None) }.ok()?;
    Some(free)
}

fn find_file(dir: &Path, name: &str, depth: u32) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    let mut subdirs = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_file() && p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(name)) {
            return Some(p);
        }
        if p.is_dir() {
            subdirs.push(p);
        }
    }
    if depth == 0 {
        return None;
    }
    subdirs.into_iter().find_map(|d| find_file(&d, name, depth - 1))
}

/// Puts the server in a job object that Windows kills when the app's process ends, even on a crash.
#[cfg(windows)]
fn tie_to_app(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::*;
    static JOB: OnceLock<usize> = OnceLock::new();
    let job = *JOB.get_or_init(|| unsafe {
        let Ok(job) = CreateJobObjectW(None, None) else { return 0 };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let _ = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const core::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        job.0 as usize
    });
    if job != 0 {
        unsafe {
            let _ = AssignProcessToJobObject(HANDLE(job as *mut core::ffi::c_void), HANDLE(child.as_raw_handle()));
        }
    }
}

#[cfg(not(windows))]
fn tie_to_app(_child: &Child) {}
