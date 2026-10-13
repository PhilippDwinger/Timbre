//! Die Systemfunktionen (WASI preview1), die Script-Gäste aus ihrer C-Bibliothek mitbringen.
//!
//! Scripts dürfen nichts am Rechner ändern: kein Schreiben, kein Netz, keine echten Dateien.
//! Es gibt nur ein **schreibgeschütztes Dateisystem im Speicher** ([`Dateisystem`]), das der Host
//! pro Sprache füllt – z. B. die Python-Standardbibliothek (`/lib/python312.zip`). Luau und Lua
//! brauchen keine Dateien, ihr Dateisystem ist leer.
//!
//! Ausgaben auf stdout/stderr (z. B. Fehlermeldungen beim Start von Python) landen zeilenweise in
//! der Konsole des Studios.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use wasmtime::{Caller, Linker, Memory};

use crate::scripting::{Kontext, Meldung};

const W: &str = "wasi_snapshot_preview1";

// Fehlercodes (errno) aus WASI preview1
const OK: i32 = 0;
const BADF: i32 = 8;
const INVAL: i32 = 28;
const ISDIR: i32 = 31;
const NOENT: i32 = 44;
const NOSYS: i32 = 52;
const NOTDIR: i32 = 54;
const NOTSUP: i32 = 58;
const PERM: i32 = 63;
const ROFS: i32 = 69;
const SPIPE: i32 = 70;

// Dateiarten
const ART_ZEICHENGERAET: u8 = 2;
const ART_ORDNER: u8 = 3;
const ART_DATEI: u8 = 4;

/// Das einzige vorab geöffnete Verzeichnis: "/" als fd 3.
const WURZEL_FD: u32 = 3;

/// Schreibgeschützte Dateien im Speicher. Pfade ohne führenden `/`, z. B. `lib/python312.zip`.
#[derive(Default)]
pub struct Dateisystem {
    dateien: Arc<BTreeMap<String, Arc<[u8]>>>,
    offen: HashMap<u32, Offen>,
    naechste: u32,
    /// Angefangene Zeilen auf stdout / stderr
    ausgabe: [Vec<u8>; 2],
}

enum Offen {
    Datei { daten: Arc<[u8]>, pos: u64 },
    Ordner { pfad: String },
}

impl Dateisystem {
    pub fn neu(dateien: Arc<BTreeMap<String, Arc<[u8]>>>) -> Self {
        Self {
            dateien,
            offen: HashMap::new(),
            naechste: WURZEL_FD + 1,
            ausgabe: Default::default(),
        }
    }

    /// Ist `pfad` ein Ordner (der Wurzelordner oder ein Präfix einer Datei)?
    fn ist_ordner(&self, pfad: &str) -> bool {
        if pfad.is_empty() {
            return true;
        }
        let praefix = format!("{pfad}/");
        self.dateien
            .range(praefix.clone()..)
            .next()
            .is_some_and(|(k, _)| k.starts_with(&praefix))
    }

    /// Direkte Einträge eines Ordners: (Name, ist Ordner)
    fn eintraege(&self, ordner: &str) -> Vec<(String, bool)> {
        let praefix = if ordner.is_empty() {
            String::new()
        } else {
            format!("{ordner}/")
        };
        let mut v: Vec<(String, bool)> = Vec::new();
        for k in self.dateien.keys().filter(|k| k.starts_with(&praefix)) {
            let rest = &k[praefix.len()..];
            let (name, ordner) = match rest.find('/') {
                Some(i) => (&rest[..i], true),
                None => (rest, false),
            };
            if v.last().map(|(n, _)| n.as_str()) != Some(name) {
                v.push((name.to_string(), ordner));
            }
        }
        v
    }

    fn pfad_von(&self, fd: u32) -> Option<String> {
        if fd == WURZEL_FD {
            return Some(String::new());
        }
        match self.offen.get(&fd)? {
            Offen::Ordner { pfad } => Some(pfad.clone()),
            Offen::Datei { .. } => None,
        }
    }
}

/// `basis` + `pfad` zusammensetzen, `.` und `..` auflösen. Nie über die Wurzel hinaus.
fn aufloesen(basis: &str, pfad: &str) -> String {
    let mut teile: Vec<&str> = if pfad.starts_with('/') {
        Vec::new()
    } else {
        basis.split('/').filter(|t| !t.is_empty()).collect()
    };
    for t in pfad.split('/') {
        match t {
            "" | "." => {}
            ".." => {
                teile.pop();
            }
            t => teile.push(t),
        }
    }
    teile.join("/")
}

fn speicher(c: &mut Caller<'_, Kontext>) -> wasmtime::Result<Memory> {
    c.get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("kein Speicher"))
}

fn lesen(c: &mut Caller<'_, Kontext>, p: i32, n: usize) -> wasmtime::Result<Vec<u8>> {
    let m = speicher(c)?;
    let mut v = vec![0; n];
    m.read(&*c, p as u32 as usize, &mut v)?;
    Ok(v)
}

fn schreiben(c: &mut Caller<'_, Kontext>, p: i32, daten: &[u8]) -> wasmtime::Result<()> {
    let m = speicher(c)?;
    m.write(&mut *c, p as u32 as usize, daten)?;
    Ok(())
}

fn u32_lesen(c: &mut Caller<'_, Kontext>, p: i32) -> wasmtime::Result<u32> {
    let b = lesen(c, p, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn text_lesen(c: &mut Caller<'_, Kontext>, p: i32, n: i32) -> wasmtime::Result<String> {
    Ok(String::from_utf8_lossy(&lesen(c, p, n as u32 as usize)?).into_owned())
}

/// iovec-Liste lesen: (Zeiger, Länge) je Eintrag
fn iovecs(c: &mut Caller<'_, Kontext>, p: i32, n: i32) -> wasmtime::Result<Vec<(i32, usize)>> {
    let mut v = Vec::new();
    for i in 0..n.max(0) {
        let zeiger = u32_lesen(c, p + i * 8)? as i32;
        let laenge = u32_lesen(c, p + i * 8 + 4)? as usize;
        v.push((zeiger, laenge));
    }
    Ok(v)
}

/// filestat (64 Bytes)
fn filestat(art: u8, groesse: u64, ino: u64) -> [u8; 64] {
    let mut b = [0u8; 64];
    b[8..16].copy_from_slice(&ino.to_le_bytes());
    b[16] = art;
    b[24..32].copy_from_slice(&1u64.to_le_bytes()); // nlink
    b[32..40].copy_from_slice(&groesse.to_le_bytes());
    b
}

/// Aus einer Datei ab `pos` in die iovecs lesen. Liefert die Anzahl gelesener Bytes.
fn in_iovecs(c: &mut Caller<'_, Kontext>, daten: &[u8], pos: u64, iov: &[(i32, usize)]) -> wasmtime::Result<usize> {
    let mut pos = pos.min(daten.len() as u64) as usize;
    let mut gesamt = 0;
    for &(zeiger, laenge) in iov {
        let n = laenge.min(daten.len() - pos);
        if n == 0 {
            break;
        }
        schreiben(c, zeiger, &daten[pos..pos + n])?;
        pos += n;
        gesamt += n;
    }
    Ok(gesamt)
}

/// Alle WASI-Funktionen registrieren.
pub fn registrieren(l: &mut Linker<Kontext>) -> wasmtime::Result<()> {
    // --- Ausgabe -----------------------------------------------------------------------------
    l.func_wrap(
        W,
        "fd_write",
        |mut c: Caller<'_, Kontext>, fd: i32, iov: i32, n: i32, geschrieben: i32| -> wasmtime::Result<i32> {
            if fd != 1 && fd != 2 {
                return Ok(if c.data().dateisystem.offen.contains_key(&(fd as u32)) {
                    ROFS
                } else {
                    BADF
                });
            }
            let mut gesamt = 0usize;
            for (zeiger, laenge) in iovecs(&mut c, iov, n)? {
                let daten = lesen(&mut c, zeiger, laenge)?;
                gesamt += daten.len();
                let k = c.data_mut();
                let puffer = &mut k.dateisystem.ausgabe[(fd - 1) as usize];
                puffer.extend_from_slice(&daten);
                // Ganze Zeilen in die Konsole (begrenzt, damit nichts überläuft)
                while let Some(i) = puffer.iter().position(|b| *b == b'\n') {
                    let zeile: Vec<u8> = puffer.drain(..=i).collect();
                    let text = String::from_utf8_lossy(&zeile[..zeile.len() - 1]).into_owned();
                    if k.meldungen.len() < 1000 {
                        k.meldungen.push(if fd == 1 {
                            Meldung::Info(text)
                        } else {
                            Meldung::Fehler(text)
                        });
                    }
                }
                if puffer.len() > 4096 {
                    puffer.clear();
                }
            }
            schreiben(&mut c, geschrieben, &(gesamt as u32).to_le_bytes())?;
            Ok(OK)
        },
    )?;
    for name in ["fd_pwrite"] {
        l.func_wrap(W, name, |_: i32, _: i32, _: i32, _: i64, _: i32| -> i32 { ROFS })?;
    }

    // --- Lesen aus dem Speicher-Dateisystem ----------------------------------------------------
    l.func_wrap(
        W,
        "fd_read",
        |mut c: Caller<'_, Kontext>, fd: i32, iov: i32, n: i32, gelesen: i32| -> wasmtime::Result<i32> {
            if fd == 0 {
                schreiben(&mut c, gelesen, &0u32.to_le_bytes())?; // stdin ist leer
                return Ok(OK);
            }
            let (daten, pos) = match c.data().dateisystem.offen.get(&(fd as u32)) {
                Some(Offen::Datei { daten, pos }) => (daten.clone(), *pos),
                Some(Offen::Ordner { .. }) => return Ok(ISDIR),
                None => return Ok(BADF),
            };
            let iov = iovecs(&mut c, iov, n)?;
            let anzahl = in_iovecs(&mut c, &daten, pos, &iov)?;
            if let Some(Offen::Datei { pos, .. }) = c.data_mut().dateisystem.offen.get_mut(&(fd as u32)) {
                *pos += anzahl as u64;
            }
            schreiben(&mut c, gelesen, &(anzahl as u32).to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "fd_pread",
        |mut c: Caller<'_, Kontext>, fd: i32, iov: i32, n: i32, ab: i64, gelesen: i32| -> wasmtime::Result<i32> {
            let daten = match c.data().dateisystem.offen.get(&(fd as u32)) {
                Some(Offen::Datei { daten, .. }) => daten.clone(),
                Some(Offen::Ordner { .. }) => return Ok(ISDIR),
                None => return Ok(BADF),
            };
            let iov = iovecs(&mut c, iov, n)?;
            let anzahl = in_iovecs(&mut c, &daten, ab as u64, &iov)?;
            schreiben(&mut c, gelesen, &(anzahl as u32).to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "fd_seek",
        |mut c: Caller<'_, Kontext>, fd: i32, abstand: i64, woher: i32, neu: i32| -> wasmtime::Result<i32> {
            let Some(Offen::Datei { daten, pos }) = c.data_mut().dateisystem.offen.get_mut(&(fd as u32)) else {
                return Ok(if fd <= 2 { SPIPE } else { BADF });
            };
            let basis = match woher {
                0 => 0i64,
                1 => *pos as i64,
                2 => daten.len() as i64,
                _ => return Ok(INVAL),
            };
            let ziel = basis + abstand;
            if ziel < 0 {
                return Ok(INVAL);
            }
            *pos = ziel as u64;
            schreiben(&mut c, neu, &(ziel as u64).to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "fd_tell",
        |mut c: Caller<'_, Kontext>, fd: i32, aus: i32| -> wasmtime::Result<i32> {
            let Some(Offen::Datei { pos, .. }) = c.data().dateisystem.offen.get(&(fd as u32)) else {
                return Ok(BADF);
            };
            let p = *pos;
            schreiben(&mut c, aus, &p.to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(W, "fd_close", |mut c: Caller<'_, Kontext>, fd: i32| -> i32 {
        if c.data_mut().dateisystem.offen.remove(&(fd as u32)).is_some() || (0..=2).contains(&fd) {
            OK
        } else {
            BADF
        }
    })?;

    // --- Infos über Dateien -------------------------------------------------------------------
    l.func_wrap(
        W,
        "fd_fdstat_get",
        |mut c: Caller<'_, Kontext>, fd: i32, aus: i32| -> wasmtime::Result<i32> {
            let art = match fd as u32 {
                0..=2 => ART_ZEICHENGERAET,
                WURZEL_FD => ART_ORDNER,
                fd => match c.data().dateisystem.offen.get(&fd) {
                    Some(Offen::Datei { .. }) => ART_DATEI,
                    Some(Offen::Ordner { .. }) => ART_ORDNER,
                    None => return Ok(BADF),
                },
            };
            let mut b = [0u8; 24];
            b[0] = art;
            b[8..16].copy_from_slice(&u64::MAX.to_le_bytes()); // Rechte (Schreiben scheitert trotzdem)
            b[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
            schreiben(&mut c, aus, &b)?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "fd_filestat_get",
        |mut c: Caller<'_, Kontext>, fd: i32, aus: i32| -> wasmtime::Result<i32> {
            let stat = match fd as u32 {
                0..=2 => filestat(ART_ZEICHENGERAET, 0, fd as u64 + 1),
                WURZEL_FD => filestat(ART_ORDNER, 0, 3),
                f => match c.data().dateisystem.offen.get(&f) {
                    Some(Offen::Datei { daten, .. }) => filestat(ART_DATEI, daten.len() as u64, f as u64 + 100),
                    Some(Offen::Ordner { .. }) => filestat(ART_ORDNER, 0, f as u64 + 100),
                    None => return Ok(BADF),
                },
            };
            schreiben(&mut c, aus, &stat)?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "path_filestat_get",
        |mut c: Caller<'_, Kontext>, fd: i32, _flags: i32, p: i32, n: i32, aus: i32| -> wasmtime::Result<i32> {
            let Some(basis) = c.data().dateisystem.pfad_von(fd as u32) else {
                return Ok(BADF);
            };
            let pfad = aufloesen(&basis, &text_lesen(&mut c, p, n)?);
            let fs = &c.data().dateisystem;
            let stat = if let Some(d) = fs.dateien.get(&pfad) {
                filestat(ART_DATEI, d.len() as u64, 1000 + pfad.len() as u64)
            } else if fs.ist_ordner(&pfad) {
                filestat(ART_ORDNER, 0, 2000 + pfad.len() as u64)
            } else {
                return Ok(NOENT);
            };
            schreiben(&mut c, aus, &stat)?;
            Ok(OK)
        },
    )?;

    // --- Öffnen -----------------------------------------------------------------------------
    l.func_wrap(
        W,
        "path_open",
        |mut c: Caller<'_, Kontext>,
         fd: i32,
         _dirflags: i32,
         p: i32,
         n: i32,
         oflags: i32,
         _rechte: i64,
         _rechte_erbe: i64,
         _fdflags: i32,
         aus: i32|
         -> wasmtime::Result<i32> {
            let Some(basis) = c.data().dateisystem.pfad_von(fd as u32) else {
                return Ok(BADF);
            };
            let pfad = aufloesen(&basis, &text_lesen(&mut c, p, n)?);
            const ERSTELLEN: i32 = 1;
            const ORDNER: i32 = 2;
            const EXKLUSIV: i32 = 4;
            const KUERZEN: i32 = 8;
            let fs = &mut c.data_mut().dateisystem;
            let offen = if let Some(daten) = fs.dateien.get(&pfad) {
                if oflags & (EXKLUSIV | KUERZEN) != 0 {
                    return Ok(ROFS);
                }
                if oflags & ORDNER != 0 {
                    return Ok(NOTDIR);
                }
                Offen::Datei {
                    daten: daten.clone(),
                    pos: 0,
                }
            } else if fs.ist_ordner(&pfad) {
                Offen::Ordner { pfad }
            } else if oflags & ERSTELLEN != 0 {
                return Ok(ROFS);
            } else {
                return Ok(NOENT);
            };
            if fs.offen.len() > 1000 {
                return Ok(PERM);
            }
            let neu = fs.naechste;
            fs.naechste += 1;
            fs.offen.insert(neu, offen);
            schreiben(&mut c, aus, &neu.to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "fd_readdir",
        |mut c: Caller<'_, Kontext>,
         fd: i32,
         puffer: i32,
         laenge: i32,
         cookie: i64,
         benutzt: i32|
         -> wasmtime::Result<i32> {
            let Some(pfad) = c.data().dateisystem.pfad_von(fd as u32) else {
                return Ok(BADF);
            };
            let eintraege = c.data().dateisystem.eintraege(&pfad);
            let mut daten: Vec<u8> = Vec::new();
            for (i, (name, ordner)) in eintraege.iter().enumerate().skip(cookie.max(0) as usize) {
                daten.extend_from_slice(&(i as u64 + 1).to_le_bytes()); // d_next
                daten.extend_from_slice(&(i as u64 + 10).to_le_bytes()); // d_ino
                daten.extend_from_slice(&(name.len() as u32).to_le_bytes());
                daten.push(if *ordner { ART_ORDNER } else { ART_DATEI });
                daten.extend_from_slice(&[0, 0, 0]);
                daten.extend_from_slice(name.as_bytes());
                if daten.len() >= laenge as usize {
                    break;
                }
            }
            daten.truncate(laenge.max(0) as usize);
            schreiben(&mut c, puffer, &daten)?;
            schreiben(&mut c, benutzt, &(daten.len() as u32).to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "fd_prestat_get",
        |mut c: Caller<'_, Kontext>, fd: i32, aus: i32| -> wasmtime::Result<i32> {
            if fd as u32 != WURZEL_FD {
                return Ok(BADF);
            }
            let mut b = [0u8; 8];
            b[4..8].copy_from_slice(&1u32.to_le_bytes()); // Name "/" hat Länge 1
            schreiben(&mut c, aus, &b)?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "fd_prestat_dir_name",
        |mut c: Caller<'_, Kontext>, fd: i32, aus: i32, n: i32| -> wasmtime::Result<i32> {
            if fd as u32 != WURZEL_FD || n < 1 {
                return Ok(BADF);
            }
            schreiben(&mut c, aus, b"/")?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "path_readlink",
        |_: i32, _: i32, _: i32, _: i32, _: i32, _: i32| -> i32 { INVAL },
    )?;

    // --- Alles, was schreibt oder ändert: verboten ---------------------------------------------
    l.func_wrap(W, "fd_fdstat_set_flags", |_: i32, _: i32| -> i32 { OK })?;
    l.func_wrap(W, "fd_renumber", |_: i32, _: i32| -> i32 { PERM })?;
    l.func_wrap(W, "fd_advise", |_: i32, _: i64, _: i64, _: i32| -> i32 { OK })?;
    l.func_wrap(W, "fd_allocate", |_: i32, _: i64, _: i64| -> i32 { ROFS })?;
    l.func_wrap(W, "fd_datasync", |_: i32| -> i32 { OK })?;
    l.func_wrap(W, "fd_sync", |_: i32| -> i32 { OK })?;
    l.func_wrap(W, "fd_filestat_set_size", |_: i32, _: i64| -> i32 { ROFS })?;
    l.func_wrap(W, "fd_filestat_set_times", |_: i32, _: i64, _: i64, _: i32| -> i32 {
        ROFS
    })?;
    l.func_wrap(W, "path_create_directory", |_: i32, _: i32, _: i32| -> i32 { ROFS })?;
    l.func_wrap(
        W,
        "path_filestat_set_times",
        |_: i32, _: i32, _: i32, _: i32, _: i64, _: i64, _: i32| -> i32 { ROFS },
    )?;
    l.func_wrap(
        W,
        "path_link",
        |_: i32, _: i32, _: i32, _: i32, _: i32, _: i32, _: i32| -> i32 { ROFS },
    )?;
    l.func_wrap(W, "path_remove_directory", |_: i32, _: i32, _: i32| -> i32 { ROFS })?;
    l.func_wrap(
        W,
        "path_rename",
        |_: i32, _: i32, _: i32, _: i32, _: i32, _: i32| -> i32 { ROFS },
    )?;
    l.func_wrap(W, "path_symlink", |_: i32, _: i32, _: i32, _: i32, _: i32| -> i32 {
        ROFS
    })?;
    l.func_wrap(W, "path_unlink_file", |_: i32, _: i32, _: i32| -> i32 { ROFS })?;

    // --- Kein Netz ------------------------------------------------------------------------------
    l.func_wrap(W, "sock_accept", |_: i32, _: i32, _: i32| -> i32 { NOTSUP })?;
    l.func_wrap(
        W,
        "sock_recv",
        |_: i32, _: i32, _: i32, _: i32, _: i32, _: i32| -> i32 { NOTSUP },
    )?;
    l.func_wrap(W, "sock_send", |_: i32, _: i32, _: i32, _: i32, _: i32| -> i32 {
        NOTSUP
    })?;
    l.func_wrap(W, "sock_shutdown", |_: i32, _: i32| -> i32 { NOTSUP })?;

    // --- Umgebung, Zeit, Zufall -----------------------------------------------------------------
    l.func_wrap(
        W,
        "environ_sizes_get",
        |mut c: Caller<'_, Kontext>, anzahl: i32, groesse: i32| -> wasmtime::Result<i32> {
            schreiben(&mut c, anzahl, &0u32.to_le_bytes())?;
            schreiben(&mut c, groesse, &0u32.to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(W, "environ_get", |_: i32, _: i32| -> i32 { OK })?;
    l.func_wrap(
        W,
        "args_sizes_get",
        |mut c: Caller<'_, Kontext>, anzahl: i32, groesse: i32| -> wasmtime::Result<i32> {
            schreiben(&mut c, anzahl, &0u32.to_le_bytes())?;
            schreiben(&mut c, groesse, &0u32.to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(W, "args_get", |_: i32, _: i32| -> i32 { OK })?;
    l.func_wrap(
        W,
        "clock_time_get",
        |mut c: Caller<'_, Kontext>, _id: i32, _genauigkeit: i64, aus: i32| -> wasmtime::Result<i32> {
            let nanos = (c.data().zeit * 1e9) as u64;
            schreiben(&mut c, aus, &nanos.to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "clock_res_get",
        |mut c: Caller<'_, Kontext>, _id: i32, aus: i32| -> wasmtime::Result<i32> {
            schreiben(&mut c, aus, &1000u64.to_le_bytes())?;
            Ok(OK)
        },
    )?;
    l.func_wrap(
        W,
        "random_get",
        |mut c: Caller<'_, Kontext>, p: i32, n: i32| -> wasmtime::Result<i32> {
            // Zufall reicht für Spiele (z. B. Hash-Startwert, random.seed); keine Kryptografie.
            let mut x = c.data().zufall;
            let mut b = vec![0u8; n.max(0) as usize];
            for z in b.iter_mut() {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                *z = x as u8;
            }
            c.data_mut().zufall = x;
            schreiben(&mut c, p, &b)?;
            Ok(OK)
        },
    )?;
    l.func_wrap(W, "poll_oneoff", |_: i32, _: i32, _: i32, _: i32| -> i32 { NOSYS })?;
    l.func_wrap(W, "sched_yield", || -> i32 { OK })?;
    l.func_wrap(W, "proc_exit", |code: i32| -> wasmtime::Result<()> {
        Err(wasmtime::Error::msg(format!("Gast hat sich beendet (Code {code})")))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfade_und_ordner() {
        assert_eq!(aufloesen("", "lib/python312.zip"), "lib/python312.zip");
        assert_eq!(aufloesen("lib", "../lib/./a.zip"), "lib/a.zip");
        assert_eq!(aufloesen("lib", "/x"), "x");
        assert_eq!(aufloesen("", "../../geheim"), "geheim");
        let mut d = BTreeMap::new();
        d.insert("lib/python312.zip".to_string(), Arc::from(&b"zip"[..]));
        d.insert("lib/x/y.py".to_string(), Arc::from(&b"y"[..]));
        let fs = Dateisystem::neu(Arc::new(d));
        assert!(fs.ist_ordner(""));
        assert!(fs.ist_ordner("lib"));
        assert!(fs.ist_ordner("lib/x"));
        assert!(!fs.ist_ordner("li"));
        assert_eq!(
            fs.eintraege("lib"),
            vec![("python312.zip".to_string(), false), ("x".to_string(), true)]
        );
    }
}
