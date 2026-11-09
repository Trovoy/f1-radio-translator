use crate::translate::{self, Job, GLOSSARY};
use crate::vision::{self, Frame, Ocr};
use std::collections::HashMap;

fn load_card() -> Frame {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/card.bgra")).unwrap();
    let w = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let h = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    Frame { w, h, bgra: bytes[8..].to_vec() }
}

#[test]
fn ocr_reads_multiviewer_cards() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    let ocr = Ocr::new().expect("ocr engine");
    let rows = ocr.recognize(&load_card().upscale2()).expect("recognize");
    for r in &rows {
        println!("{:>6.1} {:>6.1}  {}", r.top, r.left, r.text);
    }
    let cards = vision::extract_cards(rows);
    for c in &cards {
        println!("[{}] {}", c.speaker, c.text);
    }
    assert_eq!(cards.len(), 2, "expected two cards");
    assert_eq!(cards[0].speaker, "Race Engineer");
    assert_eq!(cards[1].speaker, "Lando Norris");
    assert!(cards[1].text.starts_with("Box this lap"));
    assert!(cards.iter().all(|c| vision::is_transcript(&c.text)));
}

#[test]
fn relative_time_rows_close_a_card() {
    let row = |top: f32, text: &str| vision::OcrLine { top, left: 0.0, text: text.into() };
    let cards = vision::extract_cards(vec![
        row(0.0, "We need some lift and coast now,"),
        row(20.0, "target is minus three."),
        row(40.0, "Race Engineer · 2m ago"),
    ]);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].speaker, "Race Engineer");
    assert_eq!(cards[0].text, "We need some lift and coast now, target is minus three.");
}

#[tokio::test]
#[ignore = "uses the network"]
async fn mymemory_translates() {
    let job = Job { text: "Box this lap, we're going to the hard tyre.", context: &[], glossary: GLOSSARY, speaker: "" };
    let zh = translate::translate(&translate::client(), "mymemory", &HashMap::new(), &job).await.expect("translation");
    println!("{zh}");
    assert!(zh.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));
}

#[test]
fn a_growing_card_keeps_its_id() {
    use crate::engine::{grown_from, remember};
    use std::collections::VecDeque;
    let mut recent = VecDeque::new();
    remember(&mut recent, "Race Engineer", "Box this lap", 7);
    assert_eq!(grown_from(&recent, "Race Engineer", "Box this lap, box this lap."), Some(7));
    assert_eq!(grown_from(&recent, "Lando Norris", "Box this lap, box this lap."), None, "other speaker");
    assert_eq!(grown_from(&recent, "Race Engineer", "Box this"), None, "shorter text");
    remember(&mut recent, "Race Engineer", "Box this lap, box this lap.", 7);
    assert_eq!(recent.len(), 1, "the grown line replaces the old entry");
    remember(&mut recent, "Race Engineer", "Skipped on the first pass", 0);
    assert_eq!(grown_from(&recent, "Race Engineer", "Skipped on the first pass, more"), None, "first-pass cards are not reused");
}

/// Runs OCR + card splitting on a capture given by `F1RADIO_OCR_FILE` (raw `[w][h]BGRA`).
#[test]
#[ignore = "needs F1RADIO_OCR_FILE"]
fn ocr_dump_file() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    let bytes = std::fs::read(std::env::var("F1RADIO_OCR_FILE").unwrap()).unwrap();
    let w = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let h = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let mut px = bytes[8..].to_vec();
    for p in px.chunks_exact_mut(4) {
        p[3] = 255;
    }
    let rows = Ocr::new().unwrap().recognize(&Frame { w, h, bgra: px }.upscale2()).unwrap();
    for r in &rows {
        println!("ROW {:>6.1} {:>6.1}  {}", r.top, r.left, r.text);
    }
    for c in vision::extract_cards(rows) {
        println!("CARD [{}] {}", c.speaker, c.text);
    }
}

#[test]
fn real_multiviewer_capture_drops_the_header() {
    #[cfg(windows)]
    unsafe {
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_MULTITHREADED);
    }
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/mv_real.bgra")).unwrap();
    let w = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let h = i32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let mut px = bytes[8..].to_vec();
    for p in px.chunks_exact_mut(4) {
        p[3] = 255;
    }
    let rows = Ocr::new().unwrap().recognize(&Frame { w, h, bgra: px }.upscale2()).unwrap();
    let cards = vision::extract_cards(rows);
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].speaker, "Arvid Lindblad");
    assert!(cards[0].text.starts_with("Okay, we'll manage Arvid"), "{}", cards[0].text);
    assert!(!cards[0].text.to_lowercase().contains("transcription"));
}

#[test]
fn names_round_trip_through_tokens() {
    let (t, names) = translate::protect_names("Bearman, three reds. [unclear] Leclerc. Max power.");
    assert_eq!(t, "XN0Z, three reds. [unclear] XN1Z. XN2Z power.");
    assert_eq!(translate::restore_names("XN0Z，三个红色。[听不清] X N1 Z。", &names), "Bearman，三个红色。[听不清] Leclerc。");
}

#[tokio::test]
#[ignore = "uses the network"]
async fn mymemory_keeps_driver_names() {
    let job = Job { text: "Bearman, next target, three reds. Leclerc is pitting.", context: &[], glossary: GLOSSARY, speaker: "" };
    let zh = translate::translate(&translate::client(), "mymemory", &HashMap::new(), &job).await.expect("translation");
    println!("{zh}");
    assert!(zh.contains("Bearman") && zh.contains("Leclerc"), "{zh}");
}

#[test]
fn radio_slang_is_rewritten_for_machine_translation() {
    assert_eq!(translate::pre_edit("Copy for Checo for opening lap."), "Understood. for Checo for first lap.");
    assert_eq!(translate::pre_edit("Copy that, box box."), "Understood. pit now, pit now.");
    assert_eq!(translate::pre_edit("Okay. Copy. Stay out."), "Okay. Understood. stay out on track.");
    assert_eq!(translate::pre_edit("We can copy his line."), "We can copy his line.", "mid-sentence copy keeps its meaning");
}

/// Translates sample radio lines with the DeepL key from the user's own settings file.
#[tokio::test]
#[ignore = "uses the network and the local settings"]
async fn deepl_samples_from_user_settings() {
    let path = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("com.trovoy.f1radio/settings.json");
    let s = crate::settings::Settings::load(&path);
    let cfg = s.providers.get("deepl").cloned().expect("no DeepL settings");
    for line in [
        "Copy for Checo for opening lap.",
        "Okay, Max. Box this lap. Set eight, set eight.",
        "Bearman, three reds. [unclear] Leclerc.",
        "Stay out, stay out. We are going plan B.",
        "You can use boost on the straight, lift and coast into turn one.",
    ] {
        let job = Job { text: line, context: &[], glossary: GLOSSARY, speaker: "" };
        match translate::translate(&translate::client(), "deepl", &cfg, &job).await {
            Ok(zh) => println!("OK  {line}\n    → {zh}"),
            Err(e) => println!("ERR {line}\n    → {e}"),
        }
    }
}
