//! Translation providers. Four adapter families cover all 24 services:
//! OpenAI-compatible chat, Anthropic Messages, key-only REST APIs, and
//! signed cloud APIs (Baidu MD5, Youdao SHA-256, Tencent TC3, Aliyun RPC,
//! Volcengine HMAC).

use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

pub type Fields = HashMap<String, String>;

pub struct Job<'a> {
    pub text: &'a str,
    /// Earlier lines from the same speaker, oldest first.
    pub context: &'a [String],
    pub glossary: &'a str,
    /// Whose radio channel the line came from, e.g. "Arvid Lindblad"; empty when unknown.
    pub speaker: &'a str,
}

pub const GLOSSARY: &str = "copy / copy that = 收到（确认听到，不是复制）
box / box box / box this lap = 进站（无线电指令）
stay out = 留在赛道上，不进站
lift and coast = 松油滑行
push / push now = 全力推进
track limits = 赛道界限；strike 在此语境下指警告次数，不是撞击
Boost = Boost（能量部署），2026 规则下的 ERS 部署按钮，不要译成涡轮增压
Overtake Mode = 超车模式，与 Boost 区分
recharge / harvest = 回收能量
stint = 赛段（视上下文可译为轮胎使用阶段）
set eight / set of tyres = 第八套轮胎
softs / mediums / hards / inters / wets = 软胎 / 中性胎 / 硬胎 / 半雨胎 / 全雨胎
deg = 轮胎衰退
undercut / overcut = 提前进站反超 / 晚进站反超
gap / delta = 差距 / 时间差
opening lap / lap one = 首圈
safety car / VSC = 安全车 / 虚拟安全车
blue flags = 蓝旗；double yellows = 双黄旗
pit entry / pit exit = 维修区入口 / 维修区出口
P1, P2 = 第 1 名、第 2 名
Checo = Sergio Pérez 的昵称，保留 Checo
[unclear] = [听不清]";

/// Radio slang rewritten into plain English before machine translation, which cannot read a glossary.
pub fn pre_edit(en: &str) -> String {
    static RULES: std::sync::OnceLock<Vec<(regex::Regex, &'static str)>> = std::sync::OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            (r"(?i)^\s*copy that\b[,.!]?\s*", "Understood. "),
            (r"(?i)^\s*copy\b[,.!]?\s*", "Understood. "),
            (r"(?i)([.!?]\s+)copy\b[,.!]?\s*", "${1}Understood. "),
            (r"(?i)\bbox,?\s+box\b", "pit now, pit now"),
            (r"(?i)\bbox this lap\b", "pit this lap"),
            (r"(?i)\bbox now\b", "pit now"),
            (r"(?i)\bstay out\b", "stay out on track"),
            (r"(?i)\bopening lap\b", "first lap"),
        ]
        .into_iter()
        .filter_map(|(p, r)| regex::Regex::new(p).ok().map(|re| (re, r)))
        .collect()
    });
    let mut out = en.to_string();
    for (re, rep) in rules {
        out = re.replace_all(&out, *rep).into_owned();
    }
    out.trim().to_string()
}

const SYSTEM: &str = "You translate live Formula 1 team-radio transcript lines into concise, natural Simplified Chinese. \
Keep the spoken tone. Preserve driver names, team names, car numbers, lap and turn numbers, decimals and units exactly. \
The input may contain speech-recognition errors or an unfinished sentence: use the F1 context, correct an error only when it is obvious, \
never invent facts, and return only the Chinese translation with no notes. \
Keep every driver, team and person name in English exactly as written (for example Bearman, Leclerc, Lindblad); never transliterate names into Chinese.";

pub fn client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("F1Radio/2.0")
        .build()
        .expect("http client")
}

fn f<'a>(cfg: &'a Fields, k: &str) -> &'a str {
    cfg.get(k).map(|s| s.trim()).unwrap_or("")
}

fn need<'a>(cfg: &'a Fields, k: &str, label: &str) -> Result<&'a str, String> {
    let v = f(cfg, k);
    if v.is_empty() { Err(format!("请先填写 {label}")) } else { Ok(v) }
}

fn chat_system(job: &Job) -> String {
    let mut s = format!("{SYSTEM}\n\nTerminology (English = Chinese):\n{}", job.glossary);
    if !job.speaker.trim().is_empty() {
        s.push_str(&format!(
            "\n\nThis line is from {}'s radio channel: it is either that driver speaking or their race engineer speaking to them. Use this to resolve \"you\", names and missing subjects.",
            job.speaker.trim()
        ));
    }
    if !job.context.is_empty() {
        s.push_str("\n\nEarlier lines from the same speaker, for context only (do not translate them):\n");
        for line in job.context {
            s.push_str("- ");
            s.push_str(line);
            s.push('\n');
        }
    }
    s
}

async fn send(req: reqwest::RequestBuilder) -> Result<Value, String> {
    let resp = req.send().await.map_err(|e| {
        if e.is_timeout() { "请求超时".to_string() } else { format!("网络错误：{e}") }
    })?;
    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let detail = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|v| {
                v.pointer("/error/message")
                    .or_else(|| v.pointer("/message"))
                    .or_else(|| v.pointer("/error"))
                    .and_then(|m| m.as_str().map(str::to_string))
            })
            .unwrap_or_else(|| body.chars().take(160).collect());
        return Err(format!("HTTP {}：{}", status.as_u16(), detail));
    }
    serde_json::from_str(&body).map_err(|_| format!("返回内容无法解析：{}", body.chars().take(120).collect::<String>()))
}

fn pick(v: &Value, ptr: &str) -> Result<String, String> {
    v.pointer(ptr)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "服务没有返回译文".to_string())
}

/// Default endpoints for the OpenAI-compatible family.
fn compat_base(id: &str) -> &'static str {
    match id {
        "openai" => "https://api.openai.com/v1",
        "gemini" => "https://generativelanguage.googleapis.com/v1beta/openai",
        "deepseek" => "https://api.deepseek.com/v1",
        "qwen" => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        "glm" => "https://open.bigmodel.cn/api/paas/v4",
        "kimi" => "https://api.moonshot.cn/v1",
        "doubao" => "https://ark.cn-beijing.volces.com/api/v3",
        "openrouter" => "https://openrouter.ai/api/v1",
        "silicon" => "https://api.siliconflow.cn/v1",
        "ollama" => "http://localhost:11434/v1",
        "lmstudio" => "http://localhost:1234/v1",
        _ => "",
    }
}

/// Machine-translation services cannot take a glossary, so their output gets
/// a small post-edit for the F1 terms they reliably get wrong.
fn post_edit(en: &str, zh: String) -> String {
    let lower = en.to_ascii_lowercase();
    let mut out = zh;
    let has = |w: &str| regex::Regex::new(&format!(r"\b{w}\b")).unwrap().is_match(&lower);
    if has("box") {
        for wrong in ["装箱", "拳击", "盒子", "箱子", "方框", "框"] {
            out = out.replace(wrong, "进站");
        }
    }
    if lower.contains("lift and coast") {
        for wrong in ["升降和滑行", "提升和滑行", "抬起和滑行", "举起和滑行"] {
            out = out.replace(wrong, "松油滑行");
        }
    }
    if lower.contains("track limits") {
        out = out.replace("轨道限制", "赛道界限").replace("赛道限制", "赛道界限");
    }
    if has("pit") || has("pitting") || has("pits") {
        out = out.replace("点蚀", "进站").replace("凹坑", "进站");
    }
    if lower.contains("double yellow") {
        out = out.replace("双黄色", "双黄旗");
    }
    if has("boost") {
        for wrong in ["涡轮增压器", "涡轮增压", "助推器", "推进器", "增压器", "加速器"] {
            out = out.replace(wrong, "Boost");
        }
        // DeepL treats instructions as hints and still writes "使用加速" for "use boost".
        if !out.contains("Boost") {
            out = out.replacen("加速", "Boost", 1);
        }
    }
    out
}

/// Same defaults the settings page shows, used when a saved config has no model yet.
fn default_model(id: &str) -> Option<&'static str> {
    Some(match id {
        "openai" => "gpt-6-luna",
        "claude" => "claude-haiku-5-5",
        "gemini" => "gemini-2.5-flash",
        "deepseek" => "deepseek-chat",
        "qwen" => "qwen-plus",
        "glm" => "glm-4-flash",
        "kimi" => "moonshot-v1-8k",
        "silicon" => "Qwen/Qwen2.5-7B-Instruct",
        "ollama" => "qwen2.5:7b",
        _ => return None,
    })
}

/// Driver names stay in English. Surnames always; first names are matched case-sensitively.
const NAMES: &[&str] = &[
    "Verstappen", "Hadjar", "Leclerc", "Hamilton", "Russell", "Antonelli", "Norris", "Piastri", "Alonso", "Stroll",
    "Gasly", "Colapinto", "Ocon", "Bearman", "Hulkenberg", "Hülkenberg", "Bortoleto", "Lawson", "Lindblad", "Albon",
    "Sainz", "Perez", "Pérez", "Bottas", "Tsunoda", "Doohan", "Max", "Isack", "Charles", "Lewis", "George", "Kimi",
    "Lando", "Oscar", "Fernando", "Lance", "Pierre", "Franco", "Esteban", "Oliver", "Nico", "Gabriel", "Liam", "Arvid",
    "Carlos", "Sergio", "Valtteri", "Yuki", "Jack", "Checo", "Hulk", "Seb",
];

/// Machine translation transliterates names (Bearman → 贝尔曼). Swap them for tokens it leaves alone.
pub fn protect_names(text: &str) -> (String, Vec<String>) {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(&format!(r"\b(?:{})\b", NAMES.join("|"))).unwrap());
    let mut found = Vec::new();
    let out = re
        .replace_all(text, |c: &regex::Captures| {
            found.push(c[0].to_string());
            format!("XN{}Z", found.len() - 1)
        })
        .into_owned();
    (out, found)
}

pub fn restore_names(zh: &str, names: &[String]) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?i)X\s*N\s*(\d+)\s*Z").unwrap());
    static SPACE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let space = SPACE.get_or_init(|| regex::Regex::new(r"\s+([，。！？、；：])").unwrap());
    let out = re.replace_all(zh, |c: &regex::Captures| {
        c[1].parse::<usize>().ok().and_then(|i| names.get(i)).cloned().unwrap_or_default()
    });
    space.replace_all(&out, "$1").into_owned()
}

pub async fn translate(c: &Client, id: &str, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let is_mt = matches!(id, "deepl" | "google" | "azure" | "baidu" | "youdao" | "tencent" | "aliyun" | "volc" | "mymemory" | "libre" | "deeplx");
    if !is_mt {
        return route(c, id, cfg, job).await;
    }
    let (text, names) = protect_names(&pre_edit(job.text));
    let zh = route(c, id, cfg, &Job { text: &text, context: job.context, glossary: job.glossary, speaker: job.speaker }).await?;
    Ok(post_edit(job.text, restore_names(&zh, &names)))
}

async fn route(c: &Client, id: &str, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    match id {
        "deepl" => deepl(c, cfg, job).await,
        "google" => google(c, cfg, job).await,
        "azure" => azure(c, cfg, job).await,
        "baidu" => baidu(c, cfg, job).await,
        "youdao" => youdao(c, cfg, job).await,
        "tencent" => tencent(c, cfg, job).await,
        "aliyun" => aliyun(c, cfg, job).await,
        "volc" => volc(c, cfg, job).await,
        "mymemory" => mymemory(c, cfg, job).await,
        "libre" => libre(c, cfg, job).await,
        "deeplx" => deeplx(c, cfg, job).await,
        "claude" => anthropic(c, cfg, job).await,
        _ => openai_compat(c, id, cfg, job).await,
    }
}

async fn openai_compat(c: &Client, id: &str, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let local = matches!(id, "ollama" | "lmstudio");
    let key = if local { f(cfg, "key") } else { need(cfg, "key", "API Key")? };
    let mut base = f(cfg, "url").to_string();
    if base.is_empty() {
        base = compat_base(id).to_string();
    }
    if base.is_empty() {
        return Err("请先填写 Base URL".into());
    }
    let base = base.trim_end_matches('/');
    let url = if base.ends_with("/chat/completions") {
        base.to_string()
    } else if regex::Regex::new(r"/v\d+[a-z0-9]*$|/openai$|/v\d+/?$|/compatible-mode/v1$|/paas/v4$|/api/v3$").unwrap().is_match(base) {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    };
    let model = match (f(cfg, "model"), default_model(id)) {
        ("", Some(m)) => m,
        _ => need(cfg, "model", "模型")?,
    };
    let mut system = chat_system(job);
    // Qwen3-style models think out loud by default; a live subtitle cannot wait for that.
    if model.to_ascii_lowercase().contains("qwen3") {
        system.push_str("\n/no_think");
    }
    let mut body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": job.text }
        ],
        "temperature": 0.2,
        "stream": false
    });
    if id == "openai" {
        body["max_completion_tokens"] = json!(240);
    } else {
        body["max_tokens"] = json!(240);
    }
    if id == "ollama" {
        // Keep the model in VRAM for a whole session instead of Ollama's default five minutes.
        body["keep_alive"] = json!("30m");
        body["think"] = json!(false);
    }
    let mut req = c.post(url).json(&body);
    if local {
        // The first call loads the model into VRAM, which can take well over the usual 20 s.
        req = req.timeout(Duration::from_secs(90));
    }
    if !key.is_empty() {
        req = req.bearer_auth(key);
    }
    let v = send(req).await?;
    Ok(strip_thinking(&pick(&v, "/choices/0/message/content")?))
}

/// Drops a `<think>…</think>` block some local models still emit before the answer.
pub fn strip_thinking(s: &str) -> String {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r"(?s)<think>.*?</think>|^.*?</think>").unwrap());
    re.replace_all(s, "").trim().to_string()
}

async fn anthropic(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let key = need(cfg, "key", "API Key")?;
    let base = match f(cfg, "url") { "" => "https://api.anthropic.com", s => s }.trim_end_matches('/');
    let url = if base.ends_with("/v1/messages") { base.to_string() } else { format!("{base}/v1/messages") };
    let model = match f(cfg, "model") { "" => "claude-haiku-5-5", m => m };
    let body = json!({
        "model": model,
        "max_tokens": 300,
        "system": chat_system(job),
        "messages": [{ "role": "user", "content": job.text }]
    });
    let v = send(c.post(url).header("x-api-key", key).header("anthropic-version", "2023-06-01").json(&body)).await?;
    pick(&v, "/content/0/text")
}

async fn deepl(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let key = need(cfg, "key", "API Key")?;
    let host = if key.ends_with(":fx") { "https://api-free.deepl.com" } else { "https://api.deepl.com" };
    let mut context = format!("Live Formula 1 driver-engineer radio. Terminology: {}", job.glossary.replace('\n', "; "));
    if !job.context.is_empty() {
        context.push_str(" Earlier lines: ");
        context.push_str(&job.context.join(" "));
    }
    let mut body = json!({
        "text": [job.text],
        "source_lang": "EN",
        "target_lang": "ZH-HANS",
        "context": context,
        "split_sentences": "0",
        "model_type": "quality_optimized",
        "custom_instructions": DEEPL_INSTRUCTIONS
    });
    let url = format!("{host}/v2/translate");
    let auth = format!("DeepL-Auth-Key {key}");
    match send(c.post(&url).header("Authorization", &auth).json(&body)).await {
        Ok(v) => pick(&v, "/translations/0/text"),
        // Not every plan accepts custom instructions or the quality model; retry with context only.
        Err(e) if e.starts_with("HTTP 400") => {
            if cfg!(debug_assertions) {
                eprintln!("[deepl] custom instructions rejected, retrying without: {e}");
            }
            body.as_object_mut().map(|o| {
                o.remove("custom_instructions");
                o.remove("model_type");
            });
            let v = send(c.post(&url).header("Authorization", &auth).json(&body)).await?;
            pick(&v, "/translations/0/text")
        }
        Err(e) => Err(e),
    }
}

/// DeepL's short "prompt": at most 10 instructions of 300 characters each.
const DEEPL_INSTRUCTIONS: [&str; 5] = [
    "Translate live Formula 1 driver-engineer team radio into short, natural spoken Simplified Chinese. Return only the translation.",
    "Radio terms: Copy / Understood means 收到; Box or pit means 进站; Stay out means 留在赛道上; Push means 全力推进; Set eight means 第八套轮胎; opening lap means 首圈.",
    "Keep every person's name and nickname in English exactly as written (e.g. Checo, Max, Bearman, Leclerc, Lindblad); never transliterate names.",
    "Boost is the driver's ERS energy-deployment button, write Boost（能量部署）, never 涡轮增压. Overtake Mode means 超车模式. Lift and coast means 松油滑行. Track limits means 赛道界限.",
    "The text comes from speech recognition and may contain errors or [unclear]; translate [unclear] as [听不清] and do not invent missing words.",
];

async fn google(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let key = need(cfg, "key", "API Key")?;
    let body = json!({ "q": job.text, "source": "en", "target": "zh-CN", "format": "text" });
    let v = send(c.post("https://translation.googleapis.com/language/translate/v2").query(&[("key", key)]).json(&body)).await?;
    pick(&v, "/data/translations/0/translatedText")
}

async fn azure(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let key = need(cfg, "key", "订阅密钥")?;
    let mut req = c
        .post("https://api.cognitive.microsofttranslator.com/translate?api-version=3.0&from=en&to=zh-Hans")
        .header("Ocp-Apim-Subscription-Key", key)
        .json(&json!([{ "Text": job.text }]));
    let region = f(cfg, "region");
    if !region.is_empty() {
        req = req.header("Ocp-Apim-Subscription-Region", region);
    }
    let v = send(req).await?;
    pick(&v, "/0/translations/0/text")
}

fn html_unescape(s: &str) -> String {
    s.replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

async fn mymemory(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let mut q = vec![("q", job.text), ("langpair", "en|zh-CN")];
    let email = f(cfg, "email");
    if !email.is_empty() {
        q.push(("de", email));
    }
    let v = send(c.get("https://api.mymemory.translated.net/get").query(&q)).await?;
    if let Some(code) = v.get("responseStatus").and_then(|s| s.as_i64().or_else(|| s.as_str()?.parse().ok())) {
        if code != 200 {
            let detail = v.get("responseDetails").and_then(|d| d.as_str()).unwrap_or("");
            return Err(format!("MyMemory {code}：{detail}"));
        }
    }
    Ok(html_unescape(&pick(&v, "/responseData/translatedText")?))
}

async fn libre(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let base = need(cfg, "url", "服务地址")?.trim_end_matches('/');
    let url = if base.ends_with("/translate") { base.to_string() } else { format!("{base}/translate") };
    let mut body = json!({ "q": job.text, "source": "en", "target": "zh", "format": "text" });
    let key = f(cfg, "key");
    if !key.is_empty() {
        body["api_key"] = json!(key);
    }
    let v = send(c.post(url).json(&body)).await?;
    pick(&v, "/translatedText")
}

async fn deeplx(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let url = need(cfg, "url", "服务地址")?;
    let mut req = c.post(url).json(&json!({ "text": job.text, "source_lang": "EN", "target_lang": "ZH" }));
    let token = f(cfg, "key");
    if !token.is_empty() {
        req = req.bearer_auth(token);
    }
    let v = send(req).await?;
    pick(&v, "/data")
}

fn salt() -> String {
    format!("{}", rand::random::<u32>())
}

async fn baidu(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let appid = need(cfg, "id", "APP ID")?;
    let key = need(cfg, "key", "密钥")?;
    let s = salt();
    let sign = format!("{:x}", md5::Md5::digest(format!("{appid}{}{s}{key}", job.text).as_bytes()));
    let form = [("q", job.text), ("from", "en"), ("to", "zh"), ("appid", appid), ("salt", &s), ("sign", &sign)];
    let v = send(c.post("https://fanyi-api.baidu.com/api/trans/vip/translate").form(&form)).await?;
    if let Some(code) = v.get("error_code") {
        return Err(format!("百度翻译 {}：{}", code, v.get("error_msg").and_then(|m| m.as_str()).unwrap_or("")));
    }
    let parts: Vec<String> = v
        .get("trans_result")
        .and_then(|a| a.as_array())
        .map(|a| a.iter().filter_map(|x| x.get("dst")?.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    if parts.is_empty() { Err("服务没有返回译文".into()) } else { Ok(parts.join("")) }
}

async fn youdao(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let app = need(cfg, "id", "应用 ID")?;
    let secret = need(cfg, "key", "应用密钥")?;
    let chars: Vec<char> = job.text.chars().collect();
    let input = if chars.len() > 20 {
        format!("{}{}{}", chars[..10].iter().collect::<String>(), chars.len(), chars[chars.len() - 10..].iter().collect::<String>())
    } else {
        job.text.to_string()
    };
    let s = salt();
    let now = chrono::Utc::now().timestamp().to_string();
    let sign = hex::encode(Sha256::digest(format!("{app}{input}{s}{now}{secret}").as_bytes()));
    let form = [
        ("q", job.text), ("from", "en"), ("to", "zh-CHS"), ("appKey", app), ("salt", &s),
        ("sign", &sign), ("signType", "v3"), ("curtime", &now),
    ];
    let v = send(c.post("https://openapi.youdao.com/api").form(&form)).await?;
    let code = v.get("errorCode").and_then(|e| e.as_str()).unwrap_or("0");
    if code != "0" {
        return Err(format!("有道智云错误码 {code}"));
    }
    pick(&v, "/translation/0")
}

fn hmac256(key: &[u8], msg: &str) -> Vec<u8> {
    let mut m = Hmac::<Sha256>::new_from_slice(key).unwrap();
    m.update(msg.as_bytes());
    m.finalize().into_bytes().to_vec()
}

async fn tencent(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let id = need(cfg, "id", "SecretId")?;
    let secret = need(cfg, "key", "SecretKey")?;
    let region = match f(cfg, "region") { "" => "ap-guangzhou", r => r };
    let host = "tmt.tencentcloudapi.com";
    let payload = json!({ "SourceText": job.text, "Source": "en", "Target": "zh", "ProjectId": 0 }).to_string();
    let now = chrono::Utc::now();
    let ts = now.timestamp().to_string();
    let date = now.format("%Y-%m-%d").to_string();
    let canonical = format!(
        "POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:{host}\nx-tc-action:texttranslate\n\ncontent-type;host;x-tc-action\n{}",
        hex::encode(Sha256::digest(payload.as_bytes()))
    );
    let scope = format!("{date}/tmt/tc3_request");
    let to_sign = format!("TC3-HMAC-SHA256\n{ts}\n{scope}\n{}", hex::encode(Sha256::digest(canonical.as_bytes())));
    let k = hmac256(format!("TC3{secret}").as_bytes(), &date);
    let k = hmac256(&k, "tmt");
    let k = hmac256(&k, "tc3_request");
    let sig = hex::encode(hmac256(&k, &to_sign));
    let auth = format!("TC3-HMAC-SHA256 Credential={id}/{scope}, SignedHeaders=content-type;host;x-tc-action, Signature={sig}");
    let req = c
        .post(format!("https://{host}"))
        .header("Authorization", auth)
        .header("Content-Type", "application/json; charset=utf-8")
        .header("X-TC-Action", "TextTranslate")
        .header("X-TC-Timestamp", ts)
        .header("X-TC-Version", "2018-03-21")
        .header("X-TC-Region", region)
        .body(payload);
    let v = send(req).await?;
    if let Some(msg) = v.pointer("/Response/Error/Message").and_then(|m| m.as_str()) {
        return Err(format!("腾讯翻译：{msg}"));
    }
    pick(&v, "/Response/TargetText")
}

const RFC3986: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');

fn pe(s: &str) -> String {
    utf8_percent_encode(s, RFC3986).to_string()
}

async fn aliyun(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let ak = need(cfg, "id", "AccessKey ID")?;
    let secret = need(cfg, "key", "AccessKey Secret")?;
    let nonce = format!("{}{}", rand::random::<u64>(), rand::random::<u32>());
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut p: BTreeMap<&str, &str> = BTreeMap::new();
    p.insert("Action", "TranslateGeneral");
    p.insert("Version", "2018-10-12");
    p.insert("Format", "JSON");
    p.insert("AccessKeyId", ak);
    p.insert("SignatureMethod", "HMAC-SHA1");
    p.insert("SignatureVersion", "1.0");
    p.insert("SignatureNonce", &nonce);
    p.insert("Timestamp", &ts);
    p.insert("FormatType", "text");
    p.insert("SourceLanguage", "en");
    p.insert("TargetLanguage", "zh");
    p.insert("SourceText", job.text);
    p.insert("Scene", "general");
    let canon = p.iter().map(|(k, v)| format!("{}={}", pe(k), pe(v))).collect::<Vec<_>>().join("&");
    let to_sign = format!("POST&{}&{}", pe("/"), pe(&canon));
    let mut m = Hmac::<sha1::Sha1>::new_from_slice(format!("{secret}&").as_bytes()).unwrap();
    m.update(to_sign.as_bytes());
    let sig = STANDARD.encode(m.finalize().into_bytes());
    let mut form: Vec<(&str, &str)> = p.into_iter().collect();
    form.push(("Signature", &sig));
    let v = send(c.post("https://mt.cn-hangzhou.aliyuncs.com/").form(&form)).await?;
    if let Some(code) = v.get("Code").and_then(|x| x.as_str().map(str::to_string).or_else(|| Some(x.to_string()))) {
        if code != "200" {
            return Err(format!("阿里云翻译：{}", v.get("Message").and_then(|m| m.as_str()).unwrap_or(&code)));
        }
    }
    pick(&v, "/Data/Translated")
}

async fn volc(c: &Client, cfg: &Fields, job: &Job<'_>) -> Result<String, String> {
    let ak = need(cfg, "id", "Access Key")?;
    let sk = need(cfg, "key", "Secret Key")?;
    let (host, region, service) = ("translate.volcengineapi.com", "cn-north-1", "translate");
    let query = "Action=TranslateText&Version=2020-06-01";
    let body = json!({ "SourceLanguage": "en", "TargetLanguage": "zh", "TextList": [job.text] }).to_string();
    let now = chrono::Utc::now();
    let x_date = now.format("%Y%m%dT%H%M%SZ").to_string();
    let short = now.format("%Y%m%d").to_string();
    let hash = hex::encode(Sha256::digest(body.as_bytes()));
    let canonical = format!(
        "POST\n/\n{query}\ncontent-type:application/json\nhost:{host}\nx-content-sha256:{hash}\nx-date:{x_date}\n\ncontent-type;host;x-content-sha256;x-date\n{hash}"
    );
    let scope = format!("{short}/{region}/{service}/request");
    let to_sign = format!("HMAC-SHA256\n{x_date}\n{scope}\n{}", hex::encode(Sha256::digest(canonical.as_bytes())));
    let k = hmac256(sk.as_bytes(), &short);
    let k = hmac256(&k, region);
    let k = hmac256(&k, service);
    let k = hmac256(&k, "request");
    let sig = hex::encode(hmac256(&k, &to_sign));
    let auth = format!("HMAC-SHA256 Credential={ak}/{scope}, SignedHeaders=content-type;host;x-content-sha256;x-date, Signature={sig}");
    let req = c
        .post(format!("https://{host}/?{query}"))
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .header("X-Content-Sha256", hash)
        .header("X-Date", x_date)
        .body(body);
    let v = send(req).await?;
    if let Some(msg) = v.pointer("/ResponseMetadata/Error/Message").and_then(|m| m.as_str()) {
        return Err(format!("火山翻译：{msg}"));
    }
    pick(&v, "/TranslationList/0/Translation")
}
