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
    let job = Job { text: "Box this lap, we're going to the hard tyre.", context: &[], glossary: GLOSSARY };
    let zh = translate::translate(&translate::client(), "mymemory", &HashMap::new(), &job).await.expect("translation");
    println!("{zh}");
    assert!(zh.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));
}
