//! Testszene für Meilenstein 2: Game Loop, Eingabe, Kamera und Sprite-Batching.
//!
//! Steuerung:
//!   WASD / Pfeiltasten  Kamera bewegen
//!   Mausrad / Q, E      Zoomen (zur Maus hin)
//!   Linke Maustaste     Blätter an der Maus verstreuen
//!   Leertaste           5.000 Objekte dazu
//!   R                   Zurücksetzen
//!   C                   Alles löschen

use timbre_core::{
    vec2, FrameStatistik, Input, Kamera2d, KeyCode, MouseButton, Renderer, Sprite, TexturId, Vec2,
};

use crate::texturen;

/// Spielfeld: halbe Breite/Höhe in Welteinheiten.
const ARENA: Vec2 = Vec2::new(1600.0, 1000.0);
const START_ANZAHL: usize = 2_000;

/// Holz- und Laubtöne für die Einfärbung.
const FARBEN: [[f32; 4]; 6] = [
    [0.87, 0.62, 0.38, 1.0],
    [0.70, 0.45, 0.25, 1.0],
    [0.95, 0.80, 0.55, 1.0],
    [0.45, 0.65, 0.30, 1.0],
    [0.80, 0.70, 0.20, 1.0],
    [0.85, 0.40, 0.20, 1.0],
];

struct Objekt {
    position: Vec2,
    vorher: Vec2,
    geschwindigkeit: Vec2,
    drehung: f32,
    drehung_vorher: f32,
    drehgeschwindigkeit: f32,
    groesse: f32,
    farbe: [f32; 4],
    textur: TexturId,
}

pub struct Spiel {
    kamera: Kamera2d,
    kamera_vorher: Kamera2d,
    objekte: Vec<Objekt>,
    sprites: Vec<Sprite>,
    ring: TexturId,
    blatt: TexturId,
    zufall: Zufall,
}

impl Spiel {
    pub fn new(renderer: &mut Renderer) -> Self {
        let ring = renderer.textur_laden(64, 64, &texturen::jahresringe(64));
        let blatt = renderer.textur_laden(64, 64, &texturen::blatt(64));
        let kamera = Kamera2d {
            position: Vec2::ZERO,
            zoom: 0.5,
        };
        let mut spiel = Self {
            kamera,
            kamera_vorher: kamera,
            objekte: Vec::new(),
            sprites: Vec::new(),
            ring,
            blatt,
            zufall: Zufall(0x5EED_7175_B4E1),
        };
        spiel.hinzufuegen(START_ANZAHL, None);
        spiel
    }

    /// Ein Logik-Schritt (fester Zeitschritt `dt`).
    pub fn schritt(&mut self, dt: f32, input: &Input, bildschirm: Vec2) {
        self.kamera_vorher = self.kamera;
        for o in &mut self.objekte {
            o.vorher = o.position;
            o.drehung_vorher = o.drehung;
        }

        // --- Kamera bewegen ---
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

        // --- Zoomen (Punkt unter der Maus bleibt stehen) ---
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

        // --- Objekte erzeugen / löschen ---
        if input.maustaste(MouseButton::Left) {
            let welt = self
                .kamera
                .bildschirm_zu_welt(input.maus_position(), bildschirm);
            self.hinzufuegen(20, Some(welt));
        }
        if input.taste_neu(KeyCode::Space) {
            self.hinzufuegen(5_000, None);
        }
        if input.taste_neu(KeyCode::KeyR) {
            self.objekte.clear();
            self.hinzufuegen(START_ANZAHL, None);
            self.kamera = Kamera2d {
                position: Vec2::ZERO,
                zoom: 0.5,
            };
            self.kamera_vorher = self.kamera;
        }
        if input.taste_neu(KeyCode::KeyC) {
            self.objekte.clear();
        }

        // --- Bewegen und an den Rändern abprallen ---
        for o in &mut self.objekte {
            o.position += o.geschwindigkeit * dt;
            o.drehung += o.drehgeschwindigkeit * dt;
            let rand = ARENA - Vec2::splat(o.groesse * 0.5);
            if o.position.x.abs() > rand.x {
                o.position.x = o.position.x.clamp(-rand.x, rand.x);
                o.geschwindigkeit.x = -o.geschwindigkeit.x;
            }
            if o.position.y.abs() > rand.y {
                o.position.y = o.position.y.clamp(-rand.y, rand.y);
                o.geschwindigkeit.y = -o.geschwindigkeit.y;
            }
        }
    }

    /// Sprites für den nächsten Frame bauen. `alpha` = Anteil zwischen letztem und nächstem
    /// Logik-Schritt – so bewegt sich alles flüssig, auch wenn öfter gezeichnet als gerechnet wird.
    pub fn sprites_bauen(&mut self, alpha: f32) {
        self.sprites.clear();
        self.sprites.extend(self.objekte.iter().map(|o| Sprite {
            position: o.vorher.lerp(o.position, alpha),
            groesse: Vec2::splat(o.groesse),
            drehung: o.drehung_vorher + (o.drehung - o.drehung_vorher) * alpha,
            farbe: o.farbe,
            textur: o.textur,
            uv: [0.0, 0.0, 1.0, 1.0],
        }));
    }

    pub fn sprites(&self) -> &[Sprite] {
        &self.sprites
    }

    /// Kamera zum Zeichnen, ebenfalls zwischen den Logik-Schritten überblendet.
    pub fn kamera(&self, alpha: f32) -> Kamera2d {
        self.kamera_vorher.lerp(&self.kamera, alpha)
    }

    fn hinzufuegen(&mut self, anzahl: usize, ort: Option<Vec2>) {
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
            self.objekte.push(Objekt {
                position,
                vorher: position,
                geschwindigkeit: Vec2::from_angle(winkel) * tempo,
                drehung: winkel,
                drehung_vorher: winkel,
                drehgeschwindigkeit,
                groesse,
                farbe,
                textur: if ist_blatt { self.blatt } else { self.ring },
            });
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
