use regex::Regex;
use std::sync::OnceLock;

pub struct ParsedMetadata {
    pub bpm: Option<f64>,
    pub key: Option<String>,
}

pub fn parse_filename(filename: &str, is_loop: bool) -> ParsedMetadata {
    let mut bpm = None;
    let mut key = None;

    // ==========================================
    // 1. BPM EXTRACTION
    // ==========================================

    // Pass 1: Explizit (120bpm, 120 bpm, 120_bpm) -> IMMER ERLAUBT
    static BPM_EXPLICIT: OnceLock<Regex> = OnceLock::new();
    let bpm_explicit = BPM_EXPLICIT
        .get_or_init(|| Regex::new(r"(?i)\b(\d{2,3}(?:\.\d+)?)\s*[-_]?\s*bpm\b").unwrap());

    if let Some(caps) = bpm_explicit.captures(filename) {
        if let Ok(val) = caps[1].parse::<f64>() {
            if val >= 40.0 && val <= 300.0 {
                bpm = Some(val);
            }
        }
    }

    // Pass 2: Implizit -> NUR FÜR LOOPS! (Verhindert "Snare 80")
    if bpm.is_none() && is_loop {
        // Variante A: Strikt durch _ oder - getrennt (z.B. "DrumLoop_80_Cmin.wav")
        static BPM_IMPLICIT_DELIMITED: OnceLock<Regex> = OnceLock::new();
        let bpm_delimited = BPM_IMPLICIT_DELIMITED
            .get_or_init(|| Regex::new(r"[-_]\s*(\d{2,3})\s*(?:[-_\.]|$)").unwrap());

        // Variante B: 3-stellige Zahl mit Leerzeichen (z.B. "Guitar Loop 120 Cmin.wav")
        static BPM_IMPLICIT_3DIGIT: OnceLock<Regex> = OnceLock::new();
        let bpm_3digit =
            BPM_IMPLICIT_3DIGIT.get_or_init(|| Regex::new(r"(?:^|\s)(\d{3})(?:\s|\.|$)").unwrap());

        if let Some(caps) = bpm_delimited.captures(filename) {
            if let Ok(val) = caps[1].parse::<f64>() {
                if val >= 60.0 && val <= 200.0 {
                    bpm = Some(val);
                }
            }
        } else if let Some(caps) = bpm_3digit.captures(filename) {
            if let Ok(val) = caps[1].parse::<f64>() {
                if val >= 60.0 && val <= 200.0 {
                    bpm = Some(val);
                }
            }
        }
    }

    // ==========================================
    // 2. MUSICAL KEY EXTRACTION
    // ==========================================

    static KEY_EXPLICIT: OnceLock<Regex> = OnceLock::new();
    let key_explicit = KEY_EXPLICIT.get_or_init(|| {
        Regex::new(r"(?i)\b([A-G][#b]?)\s*[-_]?\s*(min|maj|minor|major)\b").unwrap()
    });

    static KEY_SHORT: OnceLock<Regex> = OnceLock::new();
    let key_short = KEY_SHORT.get_or_init(|| Regex::new(r"\b([A-G][#b]?)(m|M)\b").unwrap());

    static CAMELOT: OnceLock<Regex> = OnceLock::new();
    let camelot = CAMELOT.get_or_init(|| Regex::new(r"(?i)\b(1[0-2]|[1-9])[AB]\b").unwrap());

    static KEY_BRACKET: OnceLock<Regex> = OnceLock::new();
    let key_bracket = KEY_BRACKET.get_or_init(|| {
        Regex::new(r"(?i)[\[\(]([A-G][#b]?)(m|min|maj|minor|major)?[\]\)]").unwrap()
    });

    static KEY_DASHED: OnceLock<Regex> = OnceLock::new();
    let key_dashed = KEY_DASHED.get_or_init(|| Regex::new(r"(?i)\s-\s([A-G][#b]?)\s-\s").unwrap());

    if let Some(caps) = key_explicit.captures(filename) {
        let note = caps[1].to_uppercase();
        let mode = caps[2].to_lowercase();
        let mode_formatted = if mode.starts_with("min") {
            "min"
        } else {
            "maj"
        };
        key = Some(format!("{} {}", note, mode_formatted));
    } else if let Some(caps) = key_short.captures(filename) {
        let note = caps[1].to_uppercase();
        let mode = if &caps[2] == "m" { "min" } else { "maj" };
        key = Some(format!("{} {}", note, mode));
    } else if let Some(caps) = camelot.captures(filename) {
        key = Some(caps[0].to_uppercase());
    } else if let Some(caps) = key_bracket.captures(filename) {
        let note = caps[1].to_uppercase();
        if let Some(mode_match) = caps.get(2) {
            let mode = mode_match.as_str().to_lowercase();
            let mode_formatted = if mode.starts_with("m") && mode != "maj" && mode != "major" {
                "min"
            } else {
                "maj"
            };
            key = Some(format!("{} {}", note, mode_formatted));
        } else {
            key = Some(note);
        }
    } else if let Some(caps) = key_dashed.captures(filename) {
        key = Some(caps[1].to_uppercase());
    }

    ParsedMetadata { bpm, key }
}
