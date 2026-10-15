//! Script-Quelltext für die Laufzeit vorbereiten.
//!
//! - **Ereignis-Namen:** Jedes Script kann in seinen Eigenschaften einstellen, welche seiner Funktionen
//!   bei ▶ (`on_start`), 60×/s (`on_update`), bei Berührung (`on_touch`) und bei Nachrichten
//!   (`on_message`) aufgerufen wird. Umgesetzt durch eine Zeile **am Ende** des Quelltexts
//!   (`on_start = starten`) – so bleiben die Zeilennummern in Fehlermeldungen gleich, und kein
//!   Sprach-Gast muss etwas davon wissen.
//! - **Live-Funktionen:** In Live-Scripts darf vor `function` / `def` das Wort `live` stehen
//!   (`live function baue(anzahl, farbe)` bzw. `live def baue(anzahl, farbe):`). Ihre Argumente
//!   erscheinen als Felder in den Eigenschaften, „▶ Run“ ruft die Funktion auf – auch ohne das
//!   Spiel zu starten. Vor dem Ausführen wird `live ` einfach entfernt.

use std::collections::BTreeMap;

use crate::projekt::Sprache;

/// Die Ereignisse: (Standard-Funktionsname, Beschriftung in den Eigenschaften)
pub const EREIGNISSE: [(&str, &str); 4] = [
    ("on_start", "Start (bei ▶)"),
    ("on_update", "Update (60×/s)"),
    ("on_touch", "Berührung"),
    ("on_message", "Nachricht"),
];

/// Eine Funktion, die im Script definiert ist.
#[derive(Clone, Debug, PartialEq)]
pub struct Funktion {
    pub name: String,
    pub parameter: Vec<String>,
    /// mit `live` davor
    pub live: bool,
}

/// Wo in einer Zeile ein `live` vor einer Funktionsdefinition steht: Byte-Bereich von `live `.
fn live_praefix(zeile: &str, sprache: Sprache) -> Option<std::ops::Range<usize>> {
    let einzug = zeile.len() - zeile.trim_start().len();
    let rest = &zeile[einzug..];
    let nach = rest.strip_prefix("live")?;
    let leer = nach.len() - nach.trim_start().len();
    if leer == 0 {
        return None; // z. B. "lively"
    }
    let danach = nach.trim_start();
    let passt = match sprache {
        Sprache::Python => danach.starts_with("def ") || danach.starts_with("async def "),
        _ => danach.starts_with("function ") || danach.starts_with("local function "),
    };
    passt.then(|| einzug..einzug + 4 + leer)
}

/// Alle Funktionen, die im Script auf oberster Ebene (Python) bzw. irgendwo als
/// `function name(...)` (Luau/Lua) definiert sind – für Vorschläge und Live-Felder.
pub fn funktionen(text: &str, sprache: Sprache) -> Vec<Funktion> {
    let mut v: Vec<Funktion> = Vec::new();
    for zeile in text.lines() {
        let (live, rest) = match live_praefix(zeile, sprache) {
            Some(r) => (true, &zeile[r.end..]),
            None => (false, zeile.trim_start()),
        };
        let kopf = match sprache {
            Sprache::Python => {
                if zeile.starts_with(char::is_whitespace) && !live {
                    continue; // nur oberste Ebene (Methoden in Klassen zählen nicht)
                }
                rest.strip_prefix("async ").unwrap_or(rest).strip_prefix("def ")
            }
            _ => rest.strip_prefix("local ").unwrap_or(rest).strip_prefix("function "),
        };
        let Some(kopf) = kopf else { continue };
        let Some(auf) = kopf.find('(') else { continue };
        let name = kopf[..auf].trim();
        if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue; // z. B. Methoden `Klasse:methode` / `t.feld`
        }
        let zu = kopf[auf..].find(')').map(|z| auf + z).unwrap_or(kopf.len());
        let parameter = kopf[auf + 1..zu]
            .split(',')
            .map(|p| {
                // Python: Standardwerte und Typangaben weg (`x: int = 3` → `x`); Luau: `x: number` → `x`
                p.split(['=', ':']).next().unwrap_or("").trim().to_string()
            })
            .filter(|p| !p.is_empty() && p != "self" && p != "..." && !p.starts_with('*'))
            .collect();
        if !v.iter().any(|f| f.name == name) {
            v.push(Funktion {
                name: name.to_string(),
                parameter,
                live,
            });
        }
    }
    v
}

/// `live ` vor Funktionsdefinitionen entfernen (Zeilen bleiben gleich).
pub fn ohne_live(text: &str, sprache: Sprache) -> String {
    let mut aus = String::with_capacity(text.len());
    for (i, zeile) in text.split('\n').enumerate() {
        if i > 0 {
            aus.push('\n');
        }
        match live_praefix(zeile, sprache) {
            Some(r) => {
                aus.push_str(&zeile[..r.start]);
                aus.push_str(&zeile[r.end..]);
            }
            None => aus.push_str(zeile),
        }
    }
    aus
}

fn nichts(sprache: Sprache) -> &'static str {
    if sprache == Sprache::Python {
        "None"
    } else {
        "nil"
    }
}

/// Quelltext so, wie er in der Laufzeit läuft: ohne `live`, plus Umleitungen der Ereignisse.
/// `ereignisse`: Ereignis → Funktionsname (leer = Ereignis aus). Fehlende Einträge = Standard.
pub fn fuer_laufzeit(text: &str, sprache: Sprache, ereignisse: &BTreeMap<String, String>) -> String {
    let mut q = ohne_live(text, sprache);
    let mut anhang = String::new();
    for (ereignis, _) in EREIGNISSE {
        let Some(name) = ereignisse.get(ereignis) else { continue };
        let name = name.trim();
        if name == ereignis {
            continue;
        }
        let ziel = if name.is_empty() { nichts(sprache) } else { name };
        anhang.push_str(&format!("{ereignis} = {ziel}\n"));
    }
    if !anhang.is_empty() {
        let kommentar = if sprache == Sprache::Python { "#" } else { "--" };
        if !q.ends_with('\n') {
            q.push('\n');
        }
        q.push_str(&format!("{kommentar} Timbre: eigene Namen der Ereignisse\n{anhang}"));
    }
    q
}

/// Text aus einem Live-Feld als Wert in der Script-Sprache: Zahl bleibt Zahl, `true`/`false`
/// werden Wahrheitswerte, leer wird nil/None, alles andere wird Text.
pub fn als_wert(eingabe: &str, sprache: Sprache) -> String {
    let t = eingabe.trim();
    let python = sprache == Sprache::Python;
    if t.is_empty() || t == "nil" || t == "None" {
        return nichts(sprache).to_string();
    }
    match t.to_ascii_lowercase().as_str() {
        "true" | "wahr" => return (if python { "True" } else { "true" }).to_string(),
        "false" | "falsch" => return (if python { "False" } else { "false" }).to_string(),
        _ => {}
    }
    if t.replace(',', ".").parse::<f64>().is_ok_and(|z| z.is_finite()) {
        return t.replace(',', ".");
    }
    let mut s = String::from("\"");
    for c in t.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// Quelltext für „▶ Run“ einer Live-Funktion: Das Script ohne seine Ereignisse, und als einziges
/// Ereignis ein `on_start`, das die Funktion mit den eingegebenen Argumenten aufruft.
pub fn fuer_live_aufruf(text: &str, sprache: Sprache, funktion: &str, argumente: &[String]) -> String {
    let mut q = ohne_live(text, sprache);
    if !q.ends_with('\n') {
        q.push('\n');
    }
    let args: Vec<String> = argumente.iter().map(|a| als_wert(a, sprache)).collect();
    let n = nichts(sprache);
    if sprache == Sprache::Python {
        q.push_str(&format!(
            "# Timbre: ▶ Run\non_update = {n}\non_touch = {n}\non_message = {n}\ndef on_start():\n    return {funktion}({})\n",
            args.join(", ")
        ));
    } else {
        q.push_str(&format!(
            "-- Timbre: ▶ Run\non_update = {n}\non_touch = {n}\non_message = {n}\non_start = function() return {funktion}({}) end\n",
            args.join(", ")
        ));
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funktionen_finden() {
        let luau = "local function hilfe()\nend\nlive function baue(anzahl, farbe: string)\nend\nfunction t.x() end\nfunction on_start() end";
        let f = funktionen(luau, Sprache::Luau);
        assert_eq!(
            f.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            vec!["hilfe", "baue", "on_start"]
        );
        assert!(f[1].live);
        assert_eq!(f[1].parameter, vec!["anzahl", "farbe"]);
        let py = "import math\nlive async def baue(anzahl: int = 3, *rest):\n    pass\ndef on_start():\n    def innen(): pass\n";
        let f = funktionen(py, Sprache::Python);
        assert_eq!(
            f.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            vec!["baue", "on_start"]
        );
        assert_eq!(f[0].parameter, vec!["anzahl"]);
        assert!(f[0].live);
    }

    #[test]
    fn live_entfernen_und_ereignisse() {
        let q = "  live function a(x)\n  end\nlocal lively = 1";
        assert_eq!(ohne_live(q, Sprache::Lua), "  function a(x)\n  end\nlocal lively = 1");
        let mut e = BTreeMap::new();
        e.insert("on_start".to_string(), "los".to_string());
        e.insert("on_update".to_string(), String::new());
        e.insert("on_touch".to_string(), "on_touch".to_string());
        let r = fuer_laufzeit("function los() end", Sprache::Luau, &e);
        assert!(r.ends_with("on_start = los\non_update = nil\n"), "{r}");
        assert!(!r.contains("on_touch ="));
        let r = fuer_laufzeit("def los(): pass", Sprache::Python, &e);
        assert!(r.ends_with("on_start = los\non_update = None\n"), "{r}");
        // Ohne Änderungen: Text bleibt gleich
        assert_eq!(fuer_laufzeit("x = 1", Sprache::Lua, &BTreeMap::new()), "x = 1");
    }

    #[test]
    fn live_aufruf() {
        assert_eq!(als_wert("3,5", Sprache::Lua), "3.5");
        assert_eq!(als_wert("wahr", Sprache::Python), "True");
        assert_eq!(als_wert("", Sprache::Python), "None");
        assert_eq!(als_wert("rot \"hell\"", Sprache::Luau), "\"rot \\\"hell\\\"\"");
        let q = fuer_live_aufruf(
            "live function baue(n, f) end",
            Sprache::Luau,
            "baue",
            &["4".into(), "rot".into()],
        );
        assert!(q.starts_with("function baue(n, f) end\n"));
        assert!(
            q.ends_with("on_start = function() return baue(4, \"rot\") end\n"),
            "{q}"
        );
        let q = fuer_live_aufruf("live def baue(n):\n    pass", Sprache::Python, "baue", &["2".into()]);
        assert!(q.contains("def on_start():\n    return baue(2)\n"), "{q}");
    }
}
