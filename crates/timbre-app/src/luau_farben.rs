//! Syntax-Hervorhebung im Script-Editor – für Luau/Lua und (mit `python = true`) für Python.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Galley, Ui};

const SCHLUESSELWOERTER: &[&str] = &[
    "and", "break", "continue", "do", "else", "elseif", "end", "export", "false", "for", "function", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "type", "until", "while",
];

const PYTHON_SCHLUESSELWOERTER: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
    "elif", "else", "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal",
    "not", "or", "pass", "raise", "return", "try", "while", "with", "yield",
];

struct Farben {
    text: Color32,
    schluessel: Color32,
    zeichenkette: Color32,
    zahl: Color32,
    kommentar: Color32,
    timbre: Color32,
    funktion: Color32,
}

fn farben(dunkel: bool) -> Farben {
    if dunkel {
        Farben {
            text: Color32::from_rgb(0xD8, 0xD2, 0xC8),
            schluessel: Color32::from_rgb(0xE0, 0x8E, 0x5A),
            zeichenkette: Color32::from_rgb(0x9C, 0xC7, 0x7A),
            zahl: Color32::from_rgb(0xD9, 0xB8, 0x5C),
            kommentar: Color32::from_rgb(0x7D, 0x77, 0x6C),
            timbre: Color32::from_rgb(0x7F, 0xB8, 0xD6),
            funktion: Color32::from_rgb(0xE8, 0xD4, 0xA2),
        }
    } else {
        Farben {
            text: Color32::from_rgb(0x2A, 0x24, 0x1E),
            schluessel: Color32::from_rgb(0xA0, 0x4A, 0x1A),
            zeichenkette: Color32::from_rgb(0x3D, 0x7A, 0x2A),
            zahl: Color32::from_rgb(0x8A, 0x6A, 0x10),
            kommentar: Color32::from_rgb(0x8C, 0x85, 0x78),
            timbre: Color32::from_rgb(0x1F, 0x6A, 0x8F),
            funktion: Color32::from_rgb(0x6A, 0x4A, 0x10),
        }
    }
}

/// Schriftgröße im Script-Editor (auch für die Zeilennummern).
pub const SCHRIFTGROESSE: f32 = 13.5;

/// Text des Editors in farbige Abschnitte zerlegen. `markiert` = Zeile (ab 1), die rot
/// hinterlegt wird (z. B. Fehlerzeile).
pub fn layout(ui: &Ui, text: &str, wrap_breite: f32, markiert: Option<usize>, python: bool) -> Arc<Galley> {
    let schluesselwoerter = if python {
        PYTHON_SCHLUESSELWOERTER
    } else {
        SCHLUESSELWOERTER
    };
    let f = farben(ui.visuals().dark_mode);
    let schrift = FontId::monospace(SCHRIFTGROESSE);
    let markierung = Color32::from_rgba_unmultiplied(0xE5, 0x6B, 0x5D, 45);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_breite;
    let mut zeile = 1usize;
    let mut abschnitt = |job: &mut LayoutJob, teil: &str, farbe: Color32| {
        let mut format = TextFormat::simple(schrift.clone(), farbe);
        if Some(zeile) == markiert {
            format.background = markierung;
        }
        job.append(teil, 0.0, format);
        zeile += teil.matches('\n').count();
    };

    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &text[i..];
        let c = bytes[i];
        let ende = if python && (rest.starts_with("\"\"\"") || rest.starts_with("'''")) {
            // Mehrzeiliger Text (oft als Kommentar benutzt)
            let e = rest[3..].find(&rest[..3]).map(|p| p + 6).unwrap_or(rest.len());
            abschnitt(&mut job, &rest[..e], f.zeichenkette);
            e
        } else if python && c == b'#' {
            let e = rest.find('\n').unwrap_or(rest.len());
            abschnitt(&mut job, &rest[..e], f.kommentar);
            e
        } else if python && rest.starts_with("--") {
            abschnitt(&mut job, "-", f.text);
            1
        } else if rest.starts_with("--[[") {
            // Block-Kommentar
            let e = rest.find("]]").map(|p| p + 2).unwrap_or(rest.len());
            abschnitt(&mut job, &rest[..e], f.kommentar);
            e
        } else if rest.starts_with("--") {
            let e = rest.find('\n').unwrap_or(rest.len());
            abschnitt(&mut job, &rest[..e], f.kommentar);
            e
        } else if c == b'"' || c == b'\'' || c == b'`' {
            let mut e = 1;
            let rb = rest.as_bytes();
            while e < rb.len() && rb[e] != c && rb[e] != b'\n' {
                e += if rb[e] == b'\\' { 2 } else { 1 };
            }
            let e = (e + 1).min(rb.len());
            abschnitt(&mut job, &rest[..e], f.zeichenkette);
            e
        } else if c.is_ascii_digit() {
            let e = rest
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '_'))
                .unwrap_or(rest.len());
            abschnitt(&mut job, &rest[..e], f.zahl);
            e
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let e = rest
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .unwrap_or(rest.len());
            let wort = &rest[..e];
            let farbe = if schluesselwoerter.contains(&wort) {
                f.schluessel
            } else if wort == "timbre" || (python && wort == "task") {
                f.timbre
            } else if rest[e..].trim_start().starts_with('(') {
                f.funktion
            } else {
                f.text
            };
            abschnitt(&mut job, wort, farbe);
            e
        } else {
            // ein Zeichen (UTF-8-sicher)
            let e = rest.chars().next().map(|ch| ch.len_utf8()).unwrap_or(1);
            abschnitt(&mut job, &rest[..e], f.text);
            e
        };
        i += ende.max(1);
    }
    ui.painter().layout_job(job)
}
