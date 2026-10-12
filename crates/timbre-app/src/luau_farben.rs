//! Syntax-Hervorhebung für Luau im Script-Editor.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Galley, Ui};

const SCHLUESSELWOERTER: &[&str] = &[
    "and", "break", "continue", "do", "else", "elseif", "end", "export", "false", "for",
    "function", "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true",
    "type", "until", "while",
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

/// Text des Editors in farbige Abschnitte zerlegen.
pub fn layout(ui: &Ui, text: &str, wrap_breite: f32) -> Arc<Galley> {
    let f = farben(ui.visuals().dark_mode);
    let schrift = FontId::monospace(13.5);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_breite;
    let abschnitt = |job: &mut LayoutJob, teil: &str, farbe: Color32| {
        job.append(teil, 0.0, TextFormat::simple(schrift.clone(), farbe));
    };

    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &text[i..];
        let c = bytes[i];
        let ende = if rest.starts_with("--[[") {
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
            let farbe = if SCHLUESSELWOERTER.contains(&wort) {
                f.schluessel
            } else if wort == "timbre" {
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
