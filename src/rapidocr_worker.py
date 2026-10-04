"""Persistent local RapidOCR worker for the MultiViewer translator."""
from __future__ import annotations

import json
import re
import sys
import traceback
from pathlib import Path

import cv2
from rapidocr import LangDet, LangRec, RapidOCR


def build_engine() -> RapidOCR:
    # PP-OCRv6 models are shipped in the RapidOCR wheel. The English settings
    # select its Latin character set while retaining the bundled v6 models.
    return RapidOCR(params={"Det.lang_type": LangDet.EN, "Rec.lang_type": LangRec.EN})


def extract_cards(result) -> list[dict[str, str]]:
    if result is None or not result.txts or result.boxes is None:
        return []

    rows = []
    for box, raw_text in zip(result.boxes, result.txts):
        text = " ".join(str(raw_text).split()).strip()
        if not text:
            continue
        top = float(min(point[1] for point in box))
        left = float(min(point[0] for point in box))
        rows.append((top, left, text))
    rows.sort(key=lambda item: (item[0], item[1]))

    # Each MultiViewer card ends with a speaker/date/time row. That gives us a
    # reliable boundary for joining wrapped radio text into one sentence.
    timestamp = re.compile(
        r"\b\d{1,2}\s+[A-Za-z]{3,9}\s+\d{4}\s*,?\s*\d{1,2}:\d{2}:\d{2}\b",
        re.IGNORECASE,
    )
    cards: list[dict[str, str]] = []
    current: list[str] = []
    for _, _, text in rows:
        match = timestamp.search(text)
        if match:
            message = " ".join(current).strip()
            speaker = text[: match.start()].strip(" \t·•|:;,—–-.")
            # Some OCR layouts split the name and timestamp into adjacent
            # boxes on the same baseline; recover a standalone name row.
            if not speaker and current:
                candidate = current[-1].strip(" \t·•|:;,—–-.")
                if re.fullmatch(r"[A-Z][A-Za-z'.-]*(?:\s+[A-Z][A-Za-z'.-]*){1,3}", candidate):
                    speaker = candidate
                    current.pop()
                    message = " ".join(current).strip()
            if message:
                cards.append({"text": message, "speaker": speaker})
            current = []
            continue
        # Ignore a date row if OCR omitted a digit but retained the month.
        if re.search(r"\b(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\b", text, re.I) and re.search(r"\b\d{1,2}:\d{2}:\d{2}\b", text):
            message = " ".join(current).strip()
            if message:
                cards.append({"text": message, "speaker": ""})
            current = []
            continue
        current.append(text)

    # Only flush a trailing card when its text is clearly complete. If the
    # card metadata has not appeared yet, the next poll can collect its tail.
    if current:
        message = " ".join(current).strip()
        if re.search(r"[.!?…]$", message) and len(message) >= 4:
            cards.append({"text": message, "speaker": ""})
    return cards


def main() -> int:
    try:
        engine = build_engine()
        print(json.dumps({"ready": True}), flush=True)
    except Exception as exc:
        print(json.dumps({"ready": False, "error": str(exc)}), flush=True)
        traceback.print_exc(file=sys.stderr)
        return 1

    for request in sys.stdin:
        path = request.strip()
        if not path:
            continue
        if path == "__quit__":
            return 0
        try:
            image = cv2.imread(str(Path(path)))
            if image is None:
                raise RuntimeError("无法读取当前截图")
            # The transcript font is small; 2x Lanczos scaling improves line
            # detection and recognition while keeping each screenshot modest.
            image = cv2.resize(image, None, fx=2, fy=2, interpolation=cv2.INTER_CUBIC)
            result = engine(image)
            print(json.dumps({"lines": extract_cards(result)}, ensure_ascii=False), flush=True)
        except Exception as exc:
            print(json.dumps({"error": str(exc)}, ensure_ascii=False), flush=True)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
