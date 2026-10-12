//! Testszene: Game Loop, Eingabe, Kamera, Sprite-Batching – und seit Meilenstein 4
//! alles als Objekte in der ECS-Welt.
//!
//! Steuerung:
//!   WASD / Pfeiltasten  Kamera bewegen
//!   Mausrad / Q, E      Zoomen (zur Maus hin)
//!   Linke Maustaste     Blätter an der Maus verstreuen
//!   Leertaste           5.000 Objekte dazu
//!   L                   Blätter unter / über die Baumscheiben legen (Ebenen)
//!   F5 / F9             Szene speichern / laden (nur nativ)
//!   R                   Zurücksetzen
//!   C                   Alles löschen

use timbre_core::welt::{self, Bewegung, Bild, Name, Transform, Welt};
use timbre_core::{
    vec2, Assets, FrameStatistik, Input, Kamera2d, KeyCode, MouseButton, Renderer, Sprite,
    TexturId, Vec2,
};

/// Die Texturen werden vorerst ins Programm eingebaut (funktioniert auch im Browser ohne Dateisystem).
/// Später lädt ein Asset-System sie zur Laufzeit – lokal oder vom Homeserver.
const JAHRESRINGE_TBR: &[u8] = include_bytes!("../../../assets/texturen/jahresringe.tbr");
const BLATT_TBR: &[u8] = include_bytes!("../../../assets/texturen/blatt.tbr");

/// Hierhin speichert F5 die Szene (relativ zum Projektordner).
#[cfg(not(target_arch = "wasm32"))]
const SZENEN_PFAD: &str = "assets/szenen/gespeichert.tbr";

/// Spielfeld: halbe Breite/Höhe in Welteinheiten.
const ARENA: Vec2 = Vec2::new(1600.0, 1000.0);
const START_ANZAHL: usize = 2_000;

const EBENE_RINGE: i32 = 0;
const EBENE_BLAETTER_OBEN: i32 = 1;
const EBENE_BLAETTER_UNTEN: i32 = -1;

/// Holz- und Laubtöne für die Einfärbung.
const FARBEN: [[f32; 4]; 6] = [
    [0.87, 0.62, 0.38, 1.0],
    [0.70, 0.45, 0.25, 1.0],
    [0.95, 0.80, 0.55, 1.0],
    [0.45, 0.65, 0.30, 1.0],
    [0.80, 0.70, 0.20, 1.0],
    [0.85, 0.40, 0.20, 1.0],
];

pub struct Spiel {
    welt: Welt,
    assets: Assets,
    kamera: Kamera2d,
    kamera_vorher: Kamera2d,
    sprites: Vec<Sprite>,
    ring: TexturId,
    blatt: TexturId,
    blaetter_unten: bool,
    zufall: Zufall,
}

impl Spiel {
    pub fn new(renderer: &mut Renderer) -> Self {
        let mut assets = Assets::default();
        let ring = assets
            .textur_laden(renderer, JAHRESRINGE_TBR)
            .expect("jahresringe.tbr kaputt");
        let blatt = assets
            .textur_laden(renderer, BLATT_TBR)
            .expect("blatt.tbr kaputt");
        let kamera = Kamera2d {
            position: Vec2::ZERO,
            zoom: 0.5,
        };
        let mut spiel = Self {
            welt: Welt::new(),
            assets,
            kamera,
            kamera_vorher: kamera,
            sprites: Vec::new(),
            ring,
            blatt,
            blaetter_unten: false,
            zufall: Zufall(0x5EED_7175_B4E1),
        };
        spiel.hinzufuegen(START_ANZAHL, None);
        spiel
    }

    /// Ein Logik-Schritt (fester Zeitschritt `dt`).
    pub fn schritt(&mut self, dt: f32, input: &Input, bildschirm: Vec2) {
        self.kamera_vorher = self.kamera;
        welt::schritt_beginnen(&mut self.welt);

        self.kamera_steuern(dt, input, bildschirm);

        // --- Objekte erzeugen / löschen ---
        if input.maustaste(MouseButton::Left) {
            let ort = self
                .kamera
                .bildschirm_zu_welt(input.maus_position(), bildschirm);
            self.hinzufuegen(20, Some(ort));
        }
        if input.taste_neu(KeyCode::Space) {
            self.hinzufuegen(5_000, None);
        }
        if input.taste_neu(KeyCode::KeyR) {
            self.welt.clear();
            self.hinzufuegen(START_ANZAHL, None);
            self.kamera = Kamera2d {
                position: Vec2::ZERO,
                zoom: 0.5,
            };
            self.kamera_vorher = self.kamera;
        }
        if input.taste_neu(KeyCode::KeyC) {
            self.welt.clear();
        }

        // --- Ebenen: Blätter über/unter die Baumscheiben ---
        if input.taste_neu(KeyCode::KeyL) {
            self.blaetter_unten = !self.blaetter_unten;
            let ebene = if self.blaetter_unten {
                EBENE_BLAETTER_UNTEN
            } else {
                EBENE_BLAETTER_OBEN
            };
            for bild in self.welt.query_mut::<&mut Bild>() {
                if bild.textur == self.blatt {
                    bild.ebene = ebene;
                }
            }
        }

        // --- Szene speichern / laden ---
        #[cfg(not(target_arch = "wasm32"))]
        {
            if input.taste_neu(KeyCode::F5) {
                match self.szene_speichern() {
                    Ok(meldung) => log::info!("{meldung}"),
                    Err(e) => log::error!("Speichern fehlgeschlagen: {e}"),
                }
            }
            if input.taste_neu(KeyCode::F9) {
                match self.szene_laden() {
                    Ok(meldung) => log::info!("{meldung}"),
                    Err(e) => log::error!("Laden fehlgeschlagen: {e}"),
                }
            }
        }

        // --- Systeme ---
        welt::bewegen(&mut self.welt, dt);
        abprallen(&mut self.welt);
    }

    fn kamera_steuern(&mut self, dt: f32, input: &Input, bildschirm: Vec2) {
        let mut richtung = Vec2::ZERO;
        if input.taste(KeyCode::KeyW) || input.taste(KeyCode::ArrowUp) {
            richtung.y += 1.0;
        }
        if input.taste(KeyCode::KeyS) || input.taste(KeyCode::ArrowDown) {
            richtung.y -= 1.0;
        }
        if input.taste(KeyCode::KeyA) || input.taste(KeyCode::ArrowLeft) {
            richtung.x -= 1.0;
        }
        if input.taste(KeyCode::KeyD) || input.taste(KeyCode::ArrowRight) {
            richtung.x += 1.0;
        }
        // Geschwindigkeit in Bildschirm-Pixeln pro Sekunde → beim Reinzoomen langsamer in der Welt.
        self.kamera.position += richtung.normalize_or_zero() * 900.0 / self.kamera.zoom * dt;

        // Zoomen: der Punkt unter der Maus bleibt stehen.
        let mut zoom_stufen = input.scroll();
        if input.taste(KeyCode::KeyE) {
            zoom_stufen += 4.0 * dt;
        }
        if input.taste(KeyCode::KeyQ) {
            zoom_stufen -= 4.0 * dt;
        }
        if zoom_stufen != 0.0 {
            let maus = input.maus_position();
            let vorher = self.kamera.bildschirm_zu_welt(maus, bildschirm);
            self.kamera.zoom = (self.kamera.zoom * 1.15f32.powf(zoom_stufen)).clamp(0.05, 20.0);
            let nachher = self.kamera.bildschirm_zu_welt(maus, bildschirm);
            self.kamera.position += vorher - nachher;
            self.kamera_vorher.position += vorher - nachher; // kein Ruckeln beim Zoomen
        }
    }

    /// Sprites für den nächsten Frame bauen. `alpha` = Anteil zwischen letztem und nächstem
    /// Logik-Schritt – so bewegt sich alles flüssig, auch wenn öfter gezeichnet als gerechnet wird.
    pub fn sprites_bauen(&mut self, alpha: f32) {
        welt::sprites_sammeln(&self.welt, alpha, &mut self.sprites);
    }

    pub fn sprites(&self) -> &[Sprite] {
        &self.sprites
    }

    /// Kamera zum Zeichnen, ebenfalls zwischen den Logik-Schritten überblendet.
    pub fn kamera(&self, alpha: f32) -> Kamera2d {
        self.kamera_vorher.lerp(&self.kamera, alpha)
    }

    fn hinzufuegen(&mut self, anzahl: usize, ort: Option<Vec2>) {
        let blatt_ebene = if self.blaetter_unten {
            EBENE_BLAETTER_UNTEN
        } else {
            EBENE_BLAETTER_OBEN
        };
        for _ in 0..anzahl {
            let z = &mut self.zufall;
            let position = match ort {
                Some(p) => p + vec2(z.bereich(-20.0, 20.0), z.bereich(-20.0, 20.0)),
                None => vec2(z.bereich(-ARENA.x, ARENA.x), z.bereich(-ARENA.y, ARENA.y)) * 0.95,
            };
            let winkel = z.bereich(0.0, std::f32::consts::TAU);
            let tempo = z.bereich(40.0, 260.0);
            let ist_blatt = z.bereich(0.0, 1.0) < 0.5;
            let farbe = FARBEN[(z.naechste() % FARBEN.len() as u64) as usize];
            let groesse = z.bereich(18.0, 56.0);
            let drehgeschwindigkeit = z.bereich(-3.0, 3.0);

            let (name, textur, ebene) = if ist_blatt {
                ("Blatt", self.blatt, blatt_ebene)
            } else {
                ("Baumscheibe", self.ring, EBENE_RINGE)
            };
            let mut bild = Bild::new(textur, Vec2::splat(groesse));
            bild.farbe = farbe;
            bild.ebene = ebene;
            self.welt.spawn((
                Name(name.into()),
                Transform {
                    position,
                    drehung: winkel,
                    skala: Vec2::ONE,
                },
                bild,
                Bewegung {
                    linear: Vec2::from_angle(winkel) * tempo,
                    drehung: drehgeschwindigkeit,
                },
            ));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn szene_speichern(&self) -> Result<String, String> {
        use timbre_core::tbr::{self, AssetId};
        let pfad = std::path::Path::new(SZENEN_PFAD);
        // Gibt es die Datei schon, behält sie ihre ID.
        let id = std::fs::read(pfad)
            .ok()
            .and_then(|d| tbr::kopf_lesen(&d).ok())
            .map(|k| k.id)
            .unwrap_or_else(AssetId::neu);
        let szene = timbre_core::szene::speichern(&self.welt, &self.assets, id, "gespeichert")?;
        if let Some(ordner) = pfad.parent() {
            std::fs::create_dir_all(ordner).map_err(|e| e.to_string())?;
        }
        std::fs::write(pfad, szene.zu_bytes()).map_err(|e| e.to_string())?;
        Ok(format!(
            "Szene mit {} Objekten gespeichert: {SZENEN_PFAD}",
            self.welt.len()
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn szene_laden(&mut self) -> Result<String, String> {
        let daten = std::fs::read(SZENEN_PFAD)
            .map_err(|e| format!("{SZENEN_PFAD}: {e} (erst mit F5 speichern)"))?;
        let szene = timbre_core::tbr::Szene::aus_bytes(&daten).map_err(|e| e.to_string())?;
        // In eine neue Welt laden: geht etwas schief, bleibt die alte unverändert.
        let mut neue_welt = Welt::new();
        let anzahl = timbre_core::szene::laden(&mut neue_welt, &self.assets, &szene)?;
        self.welt = neue_welt;
        self.blaetter_unten = self
            .welt
            .query::<&Bild>()
            .iter()
            .any(|b| b.ebene == EBENE_BLAETTER_UNTEN);
        Ok(format!(
            "Szene mit {anzahl} Objekten geladen: {SZENEN_PFAD}"
        ))
    }
}

/// System der Testszene: an den Rändern des Spielfelds abprallen.
fn abprallen(welt: &mut Welt) {
    for (t, b, bild) in welt.query_mut::<(&mut Transform, &mut Bewegung, &Bild)>() {
        let rand = ARENA - bild.groesse * t.skala * 0.5;
        if t.position.x.abs() > rand.x {
            t.position.x = t.position.x.clamp(-rand.x, rand.x);
            b.linear.x = -b.linear.x;
        }
        if t.position.y.abs() > rand.y {
            t.position.y = t.position.y.clamp(-rand.y, rand.y);
            b.linear.y = -b.linear.y;
        }
    }
}

/// Mini-Zufallsgenerator (xorshift). Reicht für eine Demo und läuft überall, auch im Browser.
struct Zufall(u64);

impl Zufall {
    fn naechste(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn bereich(&mut self, min: f32, max: f32) -> f32 {
        let t = (self.naechste() >> 40) as f32 / (1u64 << 24) as f32;
        min + (max - min) * t
    }
}

/// Zählt Bilder pro Sekunde und schreibt sie einmal pro Sekunde in den Fenstertitel.
pub struct Anzeige {
    frames: u32,
    schritte: u32,
    seit: web_time::Instant,
}

impl Default for Anzeige {
    fn default() -> Self {
        Self {
            frames: 0,
            schritte: 0,
            seit: web_time::Instant::now(),
        }
    }
}

impl Anzeige {
    pub fn frame(&mut self, schritte: u32, stat: FrameStatistik, fenster: &winit::window::Window) {
        self.frames += 1;
        self.schritte += schritte;
        let vergangen = self.seit.elapsed().as_secs_f32();
        if vergangen >= 1.0 {
            fenster.set_title(&format!(
                "{} {} · {:.0} FPS · {:.0} Logik-Schritte/s · {} Sprites · {} Draw Calls",
                timbre_core::NAME,
                timbre_core::VERSION,
                self.frames as f32 / vergangen,
                self.schritte as f32 / vergangen,
                tausender(stat.sprites),
                stat.draw_calls,
            ));
            self.frames = 0;
            self.schritte = 0;
            self.seit = web_time::Instant::now();
        }
    }
}

/// 12345 → "12.345"
fn tausender(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}
