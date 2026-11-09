//! Screen capture (GDI) and text recognition (Windows.Media.Ocr).
//! Coordinates are physical pixels; the app is per-monitor DPI aware.

use crate::settings::Region;
use regex::Regex;
use std::sync::OnceLock;

pub struct Frame {
    pub w: i32,
    pub h: i32,
    /// Top-down BGRA pixels.
    pub bgra: Vec<u8>,
}

impl Frame {
    pub fn fingerprint(&self) -> u64 {
        // FNV-1a over a sparse sample is enough to notice a changed card.
        let mut h: u64 = 0xcbf29ce484222325;
        for b in self.bgra.iter().step_by(7) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// 2x bilinear upscale; Windows OCR reads small transcript text better.
    pub fn upscale2(&self) -> Frame {
        let (w, h) = (self.w as usize, self.h as usize);
        let (nw, nh) = (w * 2, h * 2);
        let mut out = vec![0u8; nw * nh * 4];
        for y in 0..nh {
            let sy = ((y as f32 + 0.5) / 2.0 - 0.5).max(0.0);
            let y0 = (sy as usize).min(h - 1);
            let y1 = (y0 + 1).min(h - 1);
            let fy = sy - y0 as f32;
            for x in 0..nw {
                let sx = ((x as f32 + 0.5) / 2.0 - 0.5).max(0.0);
                let x0 = (sx as usize).min(w - 1);
                let x1 = (x0 + 1).min(w - 1);
                let fx = sx - x0 as f32;
                for c in 0..4 {
                    let p = |xx: usize, yy: usize| self.bgra[(yy * w + xx) * 4 + c] as f32;
                    let top = p(x0, y0) * (1.0 - fx) + p(x1, y0) * fx;
                    let bot = p(x0, y1) * (1.0 - fx) + p(x1, y1) * fx;
                    out[(y * nw + x) * 4 + c] = (top * (1.0 - fy) + bot * fy).round() as u8;
                }
            }
        }
        Frame { w: nw as i32, h: nh as i32, bgra: out }
    }
}

#[cfg(windows)]
pub fn capture(r: &Region) -> Result<Frame, String> {
    use windows::Win32::Graphics::Gdi::*;
    if r.w < 8 || r.h < 8 {
        return Err("识别区域太小".into());
    }
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, r.w, r.h);
        let old = SelectObject(mem, bmp.into());
        let blit = BitBlt(mem, 0, 0, r.w, r.h, Some(screen), r.x, r.y, SRCCOPY);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: r.w,
                biHeight: -r.h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bgra = vec![0u8; (r.w * r.h * 4) as usize];
        // GetDIBits requires the bitmap to be deselected first.
        SelectObject(mem, old);
        let rows = GetDIBits(screen, bmp, 0, r.h as u32, Some(bgra.as_mut_ptr() as _), &mut info, DIB_RGB_COLORS);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        blit.map_err(|e| format!("截屏失败：{e}"))?;
        if rows == 0 {
            return Err("截屏失败：无法读取像素".into());
        }
        // GDI leaves alpha at 0; OCR wants opaque pixels.
        for px in bgra.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Ok(Frame { w: r.w, h: r.h, bgra })
    }
}

#[cfg(not(windows))]
pub fn capture(_r: &Region) -> Result<Frame, String> {
    Err("仅支持 Windows".into())
}

pub struct OcrLine {
    pub top: f32,
    pub left: f32,
    pub text: String,
}

#[cfg(windows)]
pub struct Ocr {
    engine: windows::Media::Ocr::OcrEngine,
    pub language: String,
}

#[cfg(windows)]
impl Ocr {
    pub fn new() -> Result<Self, String> {
        use windows::core::HSTRING;
        use windows::Globalization::Language;
        use windows::Media::Ocr::OcrEngine;
        let en = Language::CreateLanguage(&HSTRING::from("en-US")).map_err(|e| e.to_string())?;
        let engine = if OcrEngine::IsLanguageSupported(&en).unwrap_or(false) {
            OcrEngine::TryCreateFromLanguage(&en)
        } else {
            OcrEngine::TryCreateFromUserProfileLanguages()
        }
        .map_err(|_| "Windows 没有可用的文字识别语言包。请在 设置 → 时间和语言 → 语言 中添加英语（美国）。".to_string())?;
        let language = engine
            .RecognizerLanguage()
            .and_then(|l| l.DisplayName())
            .map(|s| s.to_string())
            .unwrap_or_default();
        Ok(Self { engine, language })
    }

    pub fn recognize(&self, f: &Frame) -> Result<Vec<OcrLine>, String> {
        use windows::Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap};
        use windows::Storage::Streams::DataWriter;
        let run = || -> windows::core::Result<Vec<OcrLine>> {
            let writer = DataWriter::new()?;
            writer.WriteBytes(&f.bgra)?;
            let buffer = writer.DetachBuffer()?;
            let bitmap = SoftwareBitmap::CreateCopyFromBuffer(&buffer, BitmapPixelFormat::Bgra8, f.w, f.h)?;
            let result = self.engine.RecognizeAsync(&bitmap)?.get()?;
            let mut out = Vec::new();
            for line in result.Lines()? {
                let text = line.Text()?.to_string();
                let words = line.Words()?;
                let (mut top, mut left) = (f32::MAX, f32::MAX);
                for w in words {
                    let r = w.BoundingRect()?;
                    top = top.min(r.Y);
                    left = left.min(r.X);
                }
                out.push(OcrLine { top, left, text });
            }
            Ok(out)
        };
        run().map_err(|e| format!("文字识别失败：{e}"))
    }
}

#[cfg(not(windows))]
pub struct Ocr {
    pub language: String,
}

#[cfg(not(windows))]
impl Ocr {
    pub fn new() -> Result<Self, String> { Err("仅支持 Windows".into()) }
    pub fn recognize(&self, _f: &Frame) -> Result<Vec<OcrLine>, String> { Ok(vec![]) }
}

pub struct Card {
    pub speaker: String,
    pub text: String,
}

fn timestamp_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // The time part is optional: OCR sometimes drops "14:22:08" from the row.
    RE.get_or_init(|| {
        // Spaces are optional too: OCR has read "Isack Hadjar · 9 Nov 2026" as "IsockHadjar.9Nov2026".
        Regex::new(r"(?i)\d{1,2}\s*(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s*,?\s*(?:19|20)\d{2}(?:\s*[,.]?\s*\d{1,2}:\d{2}(?::\d{2})?)?").unwrap()
    })
}

/// Joins wrapped OCR rows into MultiViewer cards. Each card ends with a
/// "Name · 12 Oct 2026, 14:22:08" row, which gives a reliable boundary.
pub fn extract_cards(mut rows: Vec<OcrLine>) -> Vec<Card> {
    static NAME: OnceLock<Regex> = OnceLock::new();
    static MONTH: OnceLock<Regex> = OnceLock::new();
    static CLOCK: OnceLock<Regex> = OnceLock::new();
    static AGO: OnceLock<Regex> = OnceLock::new();
    let name = NAME.get_or_init(|| Regex::new(r"^[A-Z][A-Za-z'.-]*(?:\s+[A-Z][A-Za-z'.-]*){0,3}$").unwrap());
    let month = MONTH.get_or_init(|| Regex::new(r"(?i)\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\b").unwrap());
    let clock = CLOCK.get_or_init(|| Regex::new(r"\b\d{1,2}:\d{2}:\d{2}\b").unwrap());
    // Newer MultiViewer builds show relative time: "Race Engineer · 2m ago".
    let ago = AGO.get_or_init(|| Regex::new(r"(?i)[·•|]?\s*\b(?:\d+\s*[smh]|just now|now)\s*(?:ago)?\s*$").unwrap());
    let trim = |s: &str| s.trim_matches(|c: char| " \t·•|:;,—–-.".contains(c)).to_string();

    rows.sort_by(|a, b| a.top.partial_cmp(&b.top).unwrap().then(a.left.partial_cmp(&b.left).unwrap()));

    // MultiViewer chrome: the "AI radio transcriptions BETA" header and icon glyphs OCR reads as "o".
    static CHROME: OnceLock<Regex> = OnceLock::new();
    static LEAD: OnceLock<Regex> = OnceLock::new();
    let chrome = CHROME.get_or_init(|| Regex::new(r"(?i)radio\s*transcri|^\W*beta\W*$").unwrap());
    let lead = LEAD.get_or_init(|| Regex::new(r"^(?:[oO0©®•·|]\s+)+").unwrap());
    // Card text sits in one column right of the avatar; the metadata rows show where it starts.
    let column = rows
        .iter()
        .filter(|r| timestamp_re().is_match(&r.text) || ago.is_match(&r.text))
        .map(|r| r.left)
        .fold(f32::MAX, f32::min);
    let rows: Vec<OcrLine> = rows
        .into_iter()
        .filter(|r| !chrome.is_match(&r.text))
        // Card lines are left-aligned to that column; the avatar sits left of it, icons and tooltips far right.
        .filter(|r| column == f32::MAX || (r.left >= column - 40.0 && r.left <= column + 120.0))
        .map(|r| OcrLine { text: lead.replace(r.text.trim(), "").into_owned(), ..r })
        .filter(|r| r.text.chars().filter(|c| c.is_alphabetic()).count() >= 3)
        .collect();

    let mut cards = Vec::new();
    let mut current: Vec<String> = Vec::new();
    for row in rows {
        let text = row.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            continue;
        }
        let meta = timestamp_re().find(&text).map(|m| m.start()).or_else(|| {
            let m = ago.find(&text)?;
            // Only treat "… 2m ago" as metadata when what precedes it looks like a name.
            let head = trim(&text[..m.start()]);
            (head.is_empty() || name.is_match(&head)).then_some(m.start())
        });
        if let Some(at) = meta {
            let mut speaker = trim(&text[..at]);
            // OCR sometimes joins the card's last line and the name row into one line:
            // keep the trailing name as the speaker and give the rest back to the card text.
            if !speaker.is_empty() && !name.is_match(&speaker) {
                static TAIL: OnceLock<Regex> = OnceLock::new();
                let tail = TAIL.get_or_init(|| Regex::new(r"([A-Z][A-Za-z'’.-]*(?:\s+[A-Z][A-Za-z'’.-]*){0,2})$").unwrap());
                let head = speaker.clone();
                match tail.find(&head) {
                    Some(m) if m.start() > 0 => {
                        current.push(head[..m.start()].trim().to_string());
                        speaker = trim(m.as_str());
                    }
                    _ => {
                        current.push(head);
                        speaker = String::new();
                    }
                }
            }
            if speaker.is_empty() {
                if let Some(last) = current.last() {
                    let cand = trim(last);
                    if name.is_match(&cand) {
                        speaker = cand;
                        current.pop();
                    }
                }
            }
            let message = current.join(" ").trim().to_string();
            if !message.is_empty() {
                cards.push(Card { speaker: if speaker.is_empty() { speaker } else { canonical_speaker(&speaker) }, text: message });
            }
            current.clear();
            continue;
        }
        if month.is_match(&text) && clock.is_match(&text) {
            let message = current.join(" ").trim().to_string();
            if !message.is_empty() {
                cards.push(Card { speaker: String::new(), text: message });
            }
            current.clear();
            continue;
        }
        current.push(text);
    }
    // Text after the last name row belongs to a card cut off by the region's edge. It is left
    // out until its name row is visible, so a half-read name or date never reaches translation.
    cards
}

/// 2026 grid, used to repair OCR slips in speaker names.
const DRIVERS: &[&str] = &[
    "Max Verstappen", "Isack Hadjar", "Charles Leclerc", "Lewis Hamilton", "George Russell", "Kimi Antonelli",
    "Lando Norris", "Oscar Piastri", "Fernando Alonso", "Lance Stroll", "Pierre Gasly", "Franco Colapinto",
    "Esteban Ocon", "Oliver Bearman", "Nico Hulkenberg", "Gabriel Bortoleto", "Liam Lawson", "Arvid Lindblad",
    "Alexander Albon", "Carlos Sainz", "Sergio Perez", "Valtteri Bottas",
];

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + (ca != cb) as usize).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// "IsockHadjar" → "Isack Hadjar": the closest grid name within a few letters, else the text as read.
pub fn canonical_speaker(raw: &str) -> String {
    let compact: String = raw.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).collect();
    if compact.len() < 5 {
        return raw.trim().to_string();
    }
    let best = DRIVERS
        .iter()
        .map(|d| {
            let dc: String = d.chars().filter(|c| c.is_alphabetic()).flat_map(char::to_lowercase).collect();
            // A surname alone (e.g. "Hadjar") also counts.
            let surname: String = d.rsplit(' ').next().unwrap_or(d).to_lowercase();
            (edit_distance(&compact, &dc).min(edit_distance(&compact, &surname)), *d)
        })
        .min_by_key(|(dist, _)| *dist);
    match best {
        Some((dist, d)) if dist <= (compact.len() / 5).max(1).min(3) => d.to_string(),
        _ => raw.trim().to_string(),
    }
}

pub fn is_transcript(text: &str) -> bool {
    let len = text.chars().count();
    if !(3..=300).contains(&len) || !text.chars().any(|c| c.is_alphabetic()) {
        return false;
    }
    let digits = text.chars().filter(|c| c.is_ascii_digit()).count();
    if digits as f32 / len as f32 > 0.22 {
        return false;
    }
    let words = text.split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).count();
    words >= 2 || text.contains(['.', '!', '?'])
}
