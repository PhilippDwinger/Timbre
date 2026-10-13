//! Autovervollständigung im Script-Editor.
//!
//! Beim Tippen erscheint unter dem Cursor eine Liste passender Namen:
//! - nach `timbre.`, `task.`, `math.`, `string.`, `table.`, `script.` die Funktionen dieser Bibliothek
//!   (mit Kurzbeschreibung),
//! - sonst Schlüsselwörter, Ereignisse (`on_start` …), globale Funktionen und Namen, die im Script
//!   schon vorkommen,
//! - **in Texten** passende Namen aus dem Projekt: Sounds bei `timbre.play_sound("`, Bilder bei
//!   `timbre.set_sprite(id, "`, Objekte bei `timbre.find("`, Tasten bei `timbre.key_down("`,
//!   Nachrichten bei `timbre.broadcast("` und nach `==` / `~=` (z. B. `nachricht == "`).
//!
//! Steht der Cursor in den Klammern einer bekannten Funktion, zeigt [`hinweis`] ihre Signatur an.
//!
//! Pfeiltasten wählen, Tab oder Enter übernehmen, Esc schließt. Strg+Leertaste öffnet die Liste von Hand.

/// Ein möglicher Vorschlag.
#[derive(Clone, Debug, PartialEq)]
pub struct Eintrag {
    /// Was eingefügt wird (ohne Klammern)
    pub name: String,
    /// Signatur bzw. Kurzbeschreibung für die Anzeige
    pub info: String,
    /// Funktion → beim Übernehmen `()` anhängen; `Some(true)` = mit Argumenten (Cursor in die Klammern)
    pub klammern: Option<bool>,
    /// Name in einem Text: beim Übernehmen das schließende `"` ergänzen, falls es fehlt
    pub text: bool,
}

/// Namen aus dem Projekt, die in Texten vorgeschlagen werden.
#[derive(Clone, Debug, Default)]
pub struct Projektnamen {
    pub sounds: Vec<String>,
    pub texturen: Vec<String>,
    pub objekte: Vec<String>,
    /// Nachrichten, die irgendein Script per `broadcast("…")` verschickt
    pub nachrichten: Vec<String>,
}

impl Projektnamen {
    /// Sucht in Script-Texten nach `broadcast("name"` bzw. `broadcast('name'`.
    pub fn nachrichten_aus<'a>(texte: impl Iterator<Item = &'a str>) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for t in texte {
            for (i, _) in t.match_indices("broadcast(") {
                let rest = t[i + "broadcast(".len()..].trim_start();
                let Some(q) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                    continue;
                };
                if let Some(ende) = rest[1..].find(q) {
                    let name = &rest[1..1 + ende];
                    if !name.is_empty() && !name.contains('\n') {
                        v.push(name.to_string());
                    }
                }
            }
        }
        v.sort();
        v.dedup();
        v
    }
}

const TASTEN: &[&str] = &[
    "W", "A", "S", "D", "Space", "Enter", "Escape", "Shift", "Ctrl", "Tab", "Up", "Down", "Left", "Right", "E", "Q",
    "R", "F", "1", "2", "3",
];

/// Geöffnete Vorschlagsliste.
#[derive(Clone, Debug)]
pub struct Offen {
    /// Script-Datei, in deren Editor die Liste offen ist
    pub datei: String,
    /// Zeichen-Index, ab dem das angefangene Wort ersetzt wird
    pub anfang: usize,
    pub eintraege: Vec<Eintrag>,
    pub auswahl: usize,
}

/// (Name, Beschreibung) – Name mit `(` am Ende = Funktion mit Argumenten, mit `()` = ohne.
const TIMBRE: &[(&str, &str)] = &[
    ("spawn(", "spawn(name) → id – neues Objekt"),
    ("find(", "find(name) → id | nil – Objekt suchen"),
    ("destroy(", "destroy(id) – Objekt samt Kindern löschen"),
    ("set_position(", "set_position(id, x, y) – Weltkoordinaten"),
    ("get_position(", "get_position(id) → x, y"),
    ("set_rotation(", "set_rotation(id, winkel) – Bogenmaß"),
    ("get_rotation(", "get_rotation(id) → winkel"),
    ("set_scale(", "set_scale(id, x [, y])"),
    ("set_velocity(", "set_velocity(id, vx, vy [, drehen])"),
    ("set_sprite(", "set_sprite(id, textur, breite [, höhe, ebene])"),
    ("set_color(", "set_color(id, r, g, b [, a]) – 0 bis 1"),
    ("set_parent(", "set_parent(id, eltern | nil)"),
    ("get_parent(", "get_parent(id) → id | nil"),
    ("get_name(", "get_name(id) → name"),
    (
        "set_collision(",
        "set_collision(id, \"circle\" | \"box\" | nil [, fest, verankert])",
    ),
    ("get_touching(", "get_touching(id) → { ids }"),
    ("key_down(", "key_down(taste) → bool – gerade gedrückt"),
    ("key_pressed(", "key_pressed(taste) → bool – neu gedrückt"),
    ("mouse_down(", "mouse_down(\"left\" | \"right\" | \"middle\") → bool"),
    ("mouse_pressed(", "mouse_pressed(taste) → bool – neu gedrückt"),
    ("mouse_position()", "mouse_position() → x, y in der Welt"),
    ("set_camera(", "set_camera(x, y [, zoom])"),
    ("ui_text(", "ui_text(schlüssel, text, x, y [, größe, r, g, b, a])"),
    ("ui_clear(", "ui_clear([schlüssel]) – ohne: alle Texte"),
    (
        "broadcast(",
        "broadcast(nachricht [, daten]) → on_message in allen Scripts",
    ),
    ("play_sound(", "play_sound(name [, lautstärke, schleife])"),
    ("stop_sounds(", "stop_sounds([name]) – ohne: alle"),
    ("time()", "time() → Sekunden seit ▶"),
    ("log(", "log(...) – wie print"),
];

const TASK: &[(&str, &str)] = &[
    ("wait(", "wait([sekunden]) → vergangene Zeit – nicht in on_update"),
    ("delay(", "delay(sekunden, funktion)"),
    ("spawn(", "spawn(funktion, ...) – sofort als eigene Aufgabe"),
];

const SCRIPT: &[(&str, &str)] = &[
    ("name", "Name des Script-Objekts"),
    ("id", "Objekt-ID des Scripts"),
    ("parent", "Objekt, unter dem das Script hängt (oder nil)"),
];

const MATH: &[(&str, &str)] = &[
    ("abs(", "abs(x)"),
    ("floor(", "floor(x)"),
    ("ceil(", "ceil(x)"),
    ("round(", "round(x)"),
    ("sqrt(", "sqrt(x)"),
    ("sin(", "sin(x)"),
    ("cos(", "cos(x)"),
    ("atan2(", "atan2(y, x)"),
    ("min(", "min(a, b, ...)"),
    ("max(", "max(a, b, ...)"),
    ("clamp(", "clamp(x, min, max)"),
    ("random(", "random([m [, n]])"),
    ("pi", "3.14159…"),
    ("huge", "unendlich"),
    ("noise(", "noise(x [, y, z]) – Perlin-Rauschen"),
    ("sign(", "sign(x) → -1, 0, 1"),
    ("lerp(", "lerp(a, b, t)"),
];

const STRING: &[(&str, &str)] = &[
    ("format(", "format(muster, ...)"),
    ("len(", "len(s)"),
    ("sub(", "sub(s, von [, bis])"),
    ("upper(", "upper(s)"),
    ("lower(", "lower(s)"),
    ("rep(", "rep(s, n)"),
    ("find(", "find(s, muster)"),
    ("split(", "split(s, trenner)"),
];

const TABLE: &[(&str, &str)] = &[
    ("insert(", "insert(t, [pos,] wert)"),
    ("remove(", "remove(t [, pos])"),
    ("find(", "find(t, wert) → index"),
    ("sort(", "sort(t [, vergleich])"),
    ("concat(", "concat(t [, trenner])"),
    ("clear(", "clear(t)"),
    ("clone(", "clone(t)"),
];

const SCHLUESSELWOERTER: &[&str] = &[
    "and", "break", "continue", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local", "nil",
    "not", "or", "repeat", "return", "then", "true", "until", "while",
];

const GLOBAL: &[(&str, &str)] = &[
    ("timbre", "Engine-Funktionen – timbre."),
    ("task", "Aufgaben: task.wait / delay / spawn"),
    ("script", "dieses Script: script.parent, script.name"),
    ("shared", "Tabelle, die alle Scripts gemeinsam sehen"),
    ("math", "Mathe-Bibliothek"),
    ("string", "Text-Bibliothek"),
    ("table", "Tabellen-Bibliothek"),
    ("print(", "print(...) – in die Konsole"),
    ("pairs(", "pairs(t) – für for k, v in pairs(t)"),
    ("ipairs(", "ipairs(t) – für for i, v in ipairs(t)"),
    ("tostring(", "tostring(x)"),
    ("tonumber(", "tonumber(x)"),
    ("typeof(", "typeof(x) → \"number\", \"string\" …"),
    ("type(", "type(x) → \"number\", \"string\", \"table\" …"),
    ("pcall(", "pcall(f, ...) – Fehler abfangen"),
    ("error(", "error(nachricht)"),
    ("select(", "select(n, ...)"),
    ("on_start", "function on_start() – einmal bei ▶"),
    ("on_update", "function on_update(dt) – 60× pro Sekunde"),
    ("on_touch", "function on_touch(anderes) – Eltern-Objekt berührt etwas"),
    (
        "on_message",
        "function on_message(nachricht, daten) – nach timbre.broadcast",
    ),
];

fn eintraege(liste: &[(&str, &str)]) -> Vec<Eintrag> {
    liste
        .iter()
        .map(|(n, info)| {
            let (name, klammern) = if let Some(n) = n.strip_suffix("()") {
                (n, Some(false))
            } else if let Some(n) = n.strip_suffix('(') {
                (n, Some(true))
            } else {
                (*n, None)
            };
            Eintrag {
                name: name.to_string(),
                info: info.to_string(),
                klammern,
                text: false,
            }
        })
        .collect()
}

fn ist_namenszeichen(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Vorschläge für die Cursor-Position `cursor` (Zeichen-Index) in `text`.
///
/// Liefert (Anfang des angefangenen Worts, passende Einträge). `erzwungen` = Strg+Leertaste:
/// dann auch ohne angefangenes Wort. `lua54` = Lua statt Luau (kein `continue`, kein `typeof`).
/// `projekt` = Namen für Vorschläge in Texten.
pub fn vorschlaege(
    text: &str,
    cursor: usize,
    erzwungen: bool,
    lua54: bool,
    projekt: &Projektnamen,
) -> Option<(usize, Vec<Eintrag>)> {
    let zeichen: Vec<char> = text.chars().collect();
    let cursor = cursor.min(zeichen.len());
    if let Some(ergebnis) = text_vorschlaege(&zeichen, cursor, erzwungen, projekt) {
        return ergebnis;
    }
    // Kein Vorschlag mitten in einem Wort (rechts vom Cursor geht es weiter)
    if zeichen.get(cursor).is_some_and(|c| ist_namenszeichen(*c)) && !erzwungen {
        return None;
    }
    let mut anfang = cursor;
    while anfang > 0 && ist_namenszeichen(zeichen[anfang - 1]) {
        anfang -= 1;
    }
    let wort: String = zeichen[anfang..cursor].iter().collect();
    if wort.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None; // Zahl
    }
    // In Kommentar oder Text? Grob: Zeile bis zum Cursor ansehen.
    let zeilenanfang = zeichen[..anfang].iter().rposition(|c| *c == '\n').map_or(0, |i| i + 1);
    let zeile: String = zeichen[zeilenanfang..anfang].iter().collect();
    if zeile.contains("--") || zeile.matches('"').count() % 2 == 1 || zeile.matches('`').count() % 2 == 1 {
        return None;
    }

    // Steht vor dem Wort `bibliothek.`?
    let mut kandidaten = if anfang > 0 && zeichen[anfang - 1] == '.' {
        let mut b = anfang - 1;
        while b > 0 && ist_namenszeichen(zeichen[b - 1]) {
            b -= 1;
        }
        let bibliothek: String = zeichen[b..anfang - 1].iter().collect();
        match bibliothek.as_str() {
            "timbre" => eintraege(TIMBRE),
            "task" => eintraege(TASK),
            "script" => eintraege(SCRIPT),
            "math" => eintraege(MATH),
            "string" => eintraege(STRING),
            "table" => eintraege(TABLE),
            _ => return None,
        }
    } else {
        if wort.is_empty() && !erzwungen {
            return None;
        }
        let mut v = eintraege(GLOBAL);
        v.extend(SCHLUESSELWOERTER.iter().map(|w| Eintrag {
            name: w.to_string(),
            info: "Schlüsselwort".into(),
            klammern: None,
            text: false,
        }));
        // Namen aus dem Script selbst (ohne das Wort, das gerade getippt wird)
        let mut namen: Vec<String> = Vec::new();
        let mut aktuell = String::new();
        for (i, c) in zeichen.iter().enumerate() {
            if ist_namenszeichen(*c) {
                aktuell.push(*c);
            } else {
                if !(anfang..=cursor).contains(&i) {
                    namen.push(std::mem::take(&mut aktuell));
                }
                aktuell.clear();
            }
        }
        if !(anfang..=cursor).contains(&zeichen.len()) {
            namen.push(aktuell);
        }
        namen.sort();
        namen.dedup();
        for n in namen {
            let neu =
                n.chars().count() >= 3 && !n.chars().next().unwrap().is_ascii_digit() && !v.iter().any(|e| e.name == n);
            if neu {
                v.push(Eintrag {
                    name: n,
                    info: "aus diesem Script".into(),
                    klammern: None,
                    text: false,
                });
            }
        }
        v
    };

    if lua54 {
        kandidaten.retain(|e| e.name != "continue" && e.name != "typeof");
    }

    // Filtern: erst Treffer am Anfang, dann irgendwo im Namen (Groß-/Kleinschreibung egal)
    let klein = wort.to_lowercase();
    let mut vorne: Vec<Eintrag> = Vec::new();
    let mut mitte: Vec<Eintrag> = Vec::new();
    for e in kandidaten.drain(..) {
        let n = e.name.to_lowercase();
        if n.starts_with(&klein) {
            vorne.push(e);
        } else if klein.len() >= 2 && n.contains(&klein) {
            mitte.push(e);
        }
    }
    vorne.extend(mitte);
    // Nur noch genau das getippte Wort übrig → nichts anzeigen
    if vorne.is_empty() || (vorne.len() == 1 && vorne[0].name == wort && !erzwungen) {
        return None;
    }
    vorne.truncate(40);
    Some((anfang, vorne))
}

/// Anfang der Zeile, in der `pos` liegt (Zeichen-Index).
fn zeilenanfang(zeichen: &[char], pos: usize) -> usize {
    zeichen[..pos].iter().rposition(|c| *c == '\n').map_or(0, |i| i + 1)
}

/// Steht `pos` in einem Text (`"…"` oder `'…'`) der aktuellen Zeile? Dann: Position des öffnenden Zeichens.
fn offener_text(zeichen: &[char], pos: usize) -> Option<usize> {
    let mut offen: Option<(usize, char)> = None;
    let mut i = zeilenanfang(zeichen, pos);
    while i < pos {
        let c = zeichen[i];
        match offen {
            None if c == '-' && zeichen.get(i + 1) == Some(&'-') => return None, // Kommentar
            None if c == '"' || c == '\'' => offen = Some((i, c)),
            Some((_, q)) if c == '\\' => {
                let _ = q;
                i += 1; // Escape überspringen
            }
            Some((_, q)) if c == q => offen = None,
            _ => {}
        }
        i += 1;
    }
    offen.map(|(p, _)| p)
}

/// Welcher Funktionsaufruf umschließt `pos`? Liefert (Bibliothek, Funktion, Argument-Nummer ab 0).
/// Sucht nur in der aktuellen Zeile; Texte und verschachtelte Klammern werden übersprungen.
fn aufruf_um(zeichen: &[char], pos: usize) -> Option<(String, String, usize)> {
    let anfang = zeilenanfang(zeichen, pos);
    // Vorwärts durch die Zeile: Stapel offener Klammern mit ihrer Komma-Zahl
    let mut stapel: Vec<(usize, usize)> = Vec::new();
    let mut text: Option<char> = None;
    let mut i = anfang;
    while i < pos {
        let c = zeichen[i];
        if let Some(q) = text {
            if c == '\\' {
                i += 1;
            } else if c == q {
                text = None;
            }
        } else {
            match c {
                '"' | '\'' => text = Some(c),
                '-' if zeichen.get(i + 1) == Some(&'-') => return None,
                '(' => stapel.push((i, 0)),
                ')' => {
                    stapel.pop();
                }
                ',' => {
                    if let Some(s) = stapel.last_mut() {
                        s.1 += 1;
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    let (klammer, argument) = *stapel.last()?;
    let mut e = klammer;
    while e > anfang && zeichen[e - 1].is_whitespace() {
        e -= 1;
    }
    let mut a = e;
    while a > anfang && ist_namenszeichen(zeichen[a - 1]) {
        a -= 1;
    }
    let name: String = zeichen[a..e].iter().collect();
    if name.is_empty() {
        return None;
    }
    let mut bibliothek = String::new();
    if a > anfang && zeichen[a - 1] == '.' {
        let mut b = a - 1;
        while b > anfang && ist_namenszeichen(zeichen[b - 1]) {
            b -= 1;
        }
        bibliothek = zeichen[b..a - 1].iter().collect();
    }
    Some((bibliothek, name, argument))
}

/// Vorschläge, wenn der Cursor in einem Text steht. `None` = nicht in einem Text (normal weiter),
/// `Some(None)` = in einem Text, aber nichts Passendes.
fn text_vorschlaege(
    zeichen: &[char],
    cursor: usize,
    erzwungen: bool,
    projekt: &Projektnamen,
) -> Option<Option<(usize, Vec<Eintrag>)>> {
    let auf = offener_text(zeichen, cursor)?;
    let anfang = auf + 1;
    let wort: String = zeichen[anfang..cursor].iter().collect();

    // Was steht vor dem Text? Ein Funktionsaufruf oder ein Vergleich?
    let mut v = auf;
    while v > 0 && zeichen[v - 1] == ' ' {
        v -= 1;
    }
    let vergleich = v >= 2 && matches!((zeichen[v - 2], zeichen[v - 1]), ('=', '=') | ('~', '='));
    let liste: Vec<(String, &str)> = if vergleich {
        let mut l: Vec<(String, &str)> = projekt.nachrichten.iter().map(|n| (n.clone(), "Nachricht")).collect();
        l.extend(projekt.objekte.iter().map(|n| (n.clone(), "Objekt")));
        l
    } else {
        let Some((bibliothek, funktion, argument)) = aufruf_um(zeichen, auf) else {
            return Some(None);
        };
        if bibliothek != "timbre" && !bibliothek.is_empty() {
            return Some(None);
        }
        let namen = |v: &Vec<String>, art: &'static str| v.iter().map(|n| (n.clone(), art)).collect::<Vec<_>>();
        match (funktion.as_str(), argument) {
            ("play_sound" | "stop_sounds", 0) => namen(&projekt.sounds, "Sound"),
            ("set_sprite", 1) => namen(&projekt.texturen, "Bild"),
            ("find", 0) => namen(&projekt.objekte, "Objekt"),
            ("broadcast", 0) => namen(&projekt.nachrichten, "Nachricht"),
            ("key_down" | "key_pressed", 0) => TASTEN.iter().map(|t| (t.to_string(), "Taste")).collect(),
            ("mouse_down" | "mouse_pressed", 0) => ["left", "right", "middle"]
                .iter()
                .map(|t| (t.to_string(), "Maustaste"))
                .collect(),
            ("set_collision", 1) => vec![
                ("circle".to_string(), "Form: Kreis"),
                ("box".to_string(), "Form: Rechteck"),
            ],
            _ => return Some(None),
        }
    };

    let klein = wort.to_lowercase();
    let mut eintraege: Vec<Eintrag> = Vec::new();
    for (name, art) in liste {
        if name.to_lowercase().starts_with(&klein) && !eintraege.iter().any(|e| e.name == name) {
            eintraege.push(Eintrag {
                name,
                info: art.to_string(),
                klammern: None,
                text: true,
            });
        }
    }
    let nur_getippt = eintraege.len() == 1 && eintraege[0].name == wort;
    if eintraege.is_empty() || (nur_getippt && !erzwungen) {
        return Some(None);
    }
    eintraege.truncate(40);
    Some(Some((anfang, eintraege)))
}

/// Signatur der Funktion, in deren Klammern der Cursor steht – z. B. für `timbre.set_position(id, |`.
/// Liefert (Signatur, Argument-Nummer ab 0).
pub fn hinweis(text: &str, cursor: usize) -> Option<(String, usize)> {
    let zeichen: Vec<char> = text.chars().collect();
    let cursor = cursor.min(zeichen.len());
    let (bibliothek, funktion, argument) = aufruf_um(&zeichen, cursor)?;
    let liste = match bibliothek.as_str() {
        "timbre" => TIMBRE,
        "task" => TASK,
        "math" => MATH,
        "string" => STRING,
        "table" => TABLE,
        "" => GLOBAL,
        _ => return None,
    };
    liste
        .iter()
        .find(|(n, _)| n.trim_end_matches("()").trim_end_matches('(') == funktion && n.ends_with('('))
        .map(|(_, info)| {
            let info = if bibliothek.is_empty() {
                info.to_string()
            } else {
                format!("{bibliothek}.{info}")
            };
            (info, argument)
        })
}

/// Eintrag übernehmen: ersetzt das Wort von `anfang` bis `cursor`. Liefert die neue Cursor-Position.
pub fn einsetzen(text: &mut String, anfang: usize, cursor: usize, e: &Eintrag) -> usize {
    let byte = |i: usize| text.char_indices().nth(i).map_or(text.len(), |(b, _)| b);
    let (von, bis) = (byte(anfang), byte(cursor));
    if e.text {
        // In einem Text: Rest des Worts bis zum schließenden Anführungszeichen mit ersetzen
        let rest = &text[bis..];
        let ende = rest.find(['"', '\'', '\n', ')', ',']).unwrap_or(rest.len());
        let schliesst = rest[ende..].starts_with(['"', '\'']);
        let q = text[..von]
            .chars()
            .last()
            .filter(|c| *c == '"' || *c == '\'')
            .unwrap_or('"');
        let einfuegen = if schliesst {
            e.name.clone()
        } else {
            format!("{}{q}", e.name)
        };
        text.replace_range(von..bis + ende, &einfuegen);
        return anfang + e.name.chars().count() + 1; // hinter das schließende Anführungszeichen
    }
    // Steht schon eine Klammer hinter dem Cursor, keine zweite anhängen
    let klammer_folgt = text[bis..].starts_with('(');
    let einfuegen = match e.klammern {
        Some(_) if klammer_folgt => e.name.clone(),
        Some(_) => format!("{}()", e.name),
        None => e.name.clone(),
    };
    text.replace_range(von..bis, &einfuegen);
    let mut neu = anfang + e.name.chars().count();
    match e.klammern {
        Some(true) if !klammer_folgt => neu += 1,  // in die Klammern
        Some(false) if !klammer_folgt => neu += 2, // hinter die Klammern
        Some(_) => neu += 1,                       // hinter die vorhandene "("
        None => {}
    }
    neu
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projekt() -> Projektnamen {
        Projektnamen {
            sounds: vec!["Muenze".into(), "Jubel".into()],
            texturen: vec!["blatt".into(), "jahresringe".into()],
            objekte: vec!["Spieler".into(), "Goldstück".into()],
            nachrichten: Projektnamen::nachrichten_aus(["timbre.broadcast(\"gesammelt\")"].into_iter()),
        }
    }

    fn namen(text: &str) -> Vec<String> {
        let cursor = text.chars().count();
        vorschlaege(text, cursor, false, false, &projekt())
            .map(|(_, v)| v.into_iter().map(|e| e.name).collect())
            .unwrap_or_default()
    }

    #[test]
    fn timbre_funktionen_nach_punkt() {
        let v = namen("local x = timbre.set_p");
        assert_eq!(v[0], "set_position");
        assert!(namen("timbre.").contains(&"spawn".to_string()));
        assert!(namen("task.w").contains(&"wait".to_string()));
    }

    #[test]
    fn schluesselwoerter_und_eigene_namen() {
        let v = namen("local spieler = 1\nfunction f()\n  spi");
        assert_eq!(v, vec!["spieler"]);
        assert!(namen("fun").contains(&"function".to_string()));
        assert!(namen("on_").contains(&"on_update".to_string()));
    }

    #[test]
    fn nicht_in_kommentar_text_oder_zahl() {
        assert!(namen("-- tim").is_empty());
        assert!(namen("print(\"tim").is_empty());
        assert!(namen("x = 12").is_empty());
        assert!(namen("x = foo.").is_empty());
    }

    #[test]
    fn einsetzen_mit_klammern() {
        let mut t = "timbre.set_p".to_string();
        let (anfang, v) = vorschlaege(&t, 12, false, false, &projekt()).unwrap();
        let cursor = einsetzen(&mut t, anfang, 12, &v[0]);
        assert_eq!(t, "timbre.set_position()");
        assert_eq!(cursor, 20); // zwischen den Klammern
        let mut t = "x = timbre.ti\nweiter".to_string();
        let (anfang, v) = vorschlaege(&t, 13, false, false, &projekt()).unwrap();
        let cursor = einsetzen(&mut t, anfang, 13, &v[0]);
        assert_eq!(t, "x = timbre.time()\nweiter");
        assert_eq!(cursor, 17);
    }

    #[test]
    fn namen_in_texten() {
        assert_eq!(namen("timbre.play_sound(\"M"), vec!["Muenze"]);
        assert_eq!(namen("timbre.set_sprite(g, \"j"), vec!["jahresringe"]);
        assert_eq!(namen("local s = timbre.find(\"Sp"), vec!["Spieler"]);
        assert_eq!(namen("if nachricht == \"ges"), vec!["gesammelt"]);
        assert_eq!(namen("timbre.get_name(x) ~= \"Gold"), vec!["Goldstück"]);
        assert!(namen("timbre.key_down(\"").contains(&"Space".to_string()));
        assert_eq!(namen("timbre.set_collision(g, \"c"), vec!["circle"]);
        // Irgendein anderer Text: nichts
        assert!(namen("print(\"Mu").is_empty());
        assert!(namen("timbre.set_sprite(\"j").is_empty()); // falsches Argument
    }

    #[test]
    fn text_einsetzen_schliesst_anfuehrungszeichen() {
        let mut t = "timbre.play_sound(\"Mu".to_string();
        let c = t.chars().count();
        let (anfang, v) = vorschlaege(&t, c, false, false, &projekt()).unwrap();
        let neu = einsetzen(&mut t, anfang, c, &v[0]);
        assert_eq!(t, "timbre.play_sound(\"Muenze\"");
        assert_eq!(neu, t.chars().count());
        // Schließendes Zeichen schon da: nicht doppeln
        let mut t = "timbre.play_sound(\"Mu\", 0.5)".to_string();
        let (anfang, v) = vorschlaege(&t, 21, false, false, &projekt()).unwrap();
        einsetzen(&mut t, anfang, 21, &v[0]);
        assert_eq!(t, "timbre.play_sound(\"Muenze\", 0.5)");
    }

    #[test]
    fn signatur_hinweis() {
        let t = "timbre.set_position(spieler, ";
        let (info, arg) = hinweis(t, t.chars().count()).unwrap();
        assert!(info.starts_with("timbre.set_position(id, x, y)"), "{info}");
        assert_eq!(arg, 1);
        let t = "task.delay(2, function() print(";
        assert!(hinweis(t, t.chars().count()).unwrap().0.starts_with("print("));
        assert!(hinweis("x = 1", 5).is_none());
        let t = "timbre.ui_text(\"a,b\", ";
        assert_eq!(hinweis(t, t.chars().count()).unwrap().1, 1); // Komma im Text zählt nicht
    }
}
