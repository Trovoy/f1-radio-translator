//! Settings live in `%APPDATA%\com.trovoy.f1radio\settings.json`.
//! Secret provider fields (keys, secrets, tokens) are encrypted with
//! Windows DPAPI for the current user before they reach the disk.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Style {
    pub size: u32,
    pub font: String,
    pub align: String,
    pub lines: u32,
    pub plate: f32,
    pub show_en: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self { size: 34, font: "sans".into(), align: "center".into(), lines: 2, plate: 0.0, show_en: true }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Settings {
    pub region: Option<Region>,
    /// Provider ids in the order they are tried.
    pub chain: Vec<String>,
    /// Field values per provider id, e.g. `deepl -> { key: "…" }`.
    pub providers: HashMap<String, HashMap<String, String>>,
    pub style: Style,
    pub interval_ms: u64,
    pub skip_unchanged: bool,
    pub context_lines: u32,
    pub auto_fallback: bool,
    pub overlay: Option<WindowRect>,
    /// Where the main window was last left.
    pub main_window: Option<WindowRect>,
    /// Folder for the local engine and models; none means the app's own data folder.
    pub model_dir: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            region: None,
            chain: vec!["mymemory".into()],
            providers: HashMap::new(),
            style: Style::default(),
            interval_ms: 1200,
            skip_unchanged: true,
            context_lines: 2,
            auto_fallback: true,
            overlay: None,
            main_window: None,
            model_dir: None,
        }
    }
}

fn is_secret(field: &str) -> bool {
    let f = field.to_ascii_lowercase();
    f.contains("key") || f.contains("secret") || f.contains("token")
}

const PREFIX: &str = "dpapi:";

impl Settings {
    pub fn load(path: &PathBuf) -> Self {
        let Ok(text) = fs::read_to_string(path) else { return Self::default() };
        let mut s: Settings = serde_json::from_str(&text).unwrap_or_default();
        for fields in s.providers.values_mut() {
            for (k, v) in fields.iter_mut() {
                if is_secret(k) {
                    if let Some(enc) = v.strip_prefix(PREFIX) {
                        *v = dpapi::unprotect(enc).unwrap_or_default();
                    }
                }
            }
        }
        if s.chain.is_empty() {
            s.chain = vec!["mymemory".into()];
        }
        s
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        let mut disk = self.clone();
        for fields in disk.providers.values_mut() {
            for (k, v) in fields.iter_mut() {
                if is_secret(k) && !v.is_empty() {
                    *v = format!("{PREFIX}{}", dpapi::protect(v)?);
                }
            }
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&disk).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
mod dpapi {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    fn run(input: &[u8], protect: bool) -> Result<Vec<u8>, String> {
        let data_in = CRYPT_INTEGER_BLOB { cbData: input.len() as u32, pbData: input.as_ptr() as *mut u8 };
        let mut data_out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            if protect {
                CryptProtectData(&data_in, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut data_out)
            } else {
                CryptUnprotectData(&data_in, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut data_out)
            }
            .map_err(|e| e.to_string())?;
            let out = std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(data_out.pbData as _)));
            Ok(out)
        }
    }

    pub fn protect(plain: &str) -> Result<String, String> {
        Ok(STANDARD.encode(run(plain.as_bytes(), true)?))
    }

    pub fn unprotect(enc: &str) -> Result<String, String> {
        let bytes = STANDARD.decode(enc).map_err(|e| e.to_string())?;
        String::from_utf8(run(&bytes, false)?).map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod dpapi {
    pub fn protect(plain: &str) -> Result<String, String> { Ok(plain.to_string()) }
    pub fn unprotect(enc: &str) -> Result<String, String> { Ok(enc.to_string()) }
}
