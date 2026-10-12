//! Timbre Studio: der Editor.
//!
//! Zwei Modi wie in Roblox Studio:
//! - **Bearbeiten**: Objekte auswählen, verschieben, Eigenschaften ändern, Scripts schreiben.
//! - **Läuft** (▶ / F5): Scripts laufen. Bei ■ Stopp wird die Szene auf den Stand von vor
//!   dem Start zurückgesetzt – Änderungen während des Laufens gehen also verloren.

use egui::{Color32, Key, Modifiers, PointerButton, RichText, Sense, Stroke};
use timbre_core::tbr::{self, AssetId};
use timbre_core::welt::{self, Bewegung, Bild, Name, Objekt, Transform, Welt};
use timbre_core::{
    szene, vec2, Assets, Bereich, FrameStatistik, Input, Kamera2d, KeyCode, Renderer, Sprite,
    TexturId, Vec2,
};

use crate::luau_farben;
use crate::projekt::{self, ProjektInhalt};

#[cfg(not(target_arch = "wasm32"))]
use timbre_core::scripting::{Meldung, ScriptLaufzeit, ScriptMotor};

/// Texturen sind vorerst eingebaut (später: Asset-Ordner des Projekts).
const TEXTUREN: [&[u8]; 2] = [
    include_bytes!("../../../assets/texturen/jahresringe.tbr"),
    include_bytes!("../../../assets/texturen/blatt.tbr"),
];

/// Der Luau-Interpreter als WebAssembly-Modul (gebaut mit tools/luau-gast/bauen.sh).
#[cfg(not(target_arch = "wasm32"))]
const LUAU_GAST: &[u8] = include_bytes!("../../../assets/scripting/luau.wasm");

const NEUES_SCRIPT: &str = "-- Neues Script\n\nfunction on_start()\n\tprint(\"Hallo aus Timbre!\")\nend\n\nfunction on_update(dt)\n\nend\n";
const MAX_KONSOLE: usize = 2000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Modus {
    Bearbeiten,
    Laeuft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Szene,
    Script(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Art {
    Info,
    System,
    Fehler,
}

struct Zeile {
    art: Art,
    text: String,
}

struct ScriptTab {
    name: String,
    text: String,
    /// Stand auf der Festplatte – zum Erkennen ungespeicherter Änderungen
    gespeichert: String,
}

/// Das Übersetzen des Luau-Moduls dauert einen Moment – passiert im Hintergrund.
#[cfg(not(target_arch = "wasm32"))]
enum Motor {
    Laedt(std::sync::mpsc::Receiver<Result<ScriptMotor, String>>),
    Bereit(ScriptMotor),
    Kaputt(String),
}

pub struct Studio {
    projekt_name: String,
    #[cfg(not(target_arch = "wasm32"))]
    ordner: std::path::PathBuf,
    welt: Welt,
    assets: Assets,
    szene_id: Option<AssetId>,
    scripts: Vec<ScriptTab>,
    tab: Tab,
    modus: Modus,
    schnappschuss: Option<tbr::Szene>,
    kamera_vor_start: Kamera2d,
    #[cfg(not(target_arch = "wasm32"))]
    motor: Motor,
    #[cfg(not(target_arch = "wasm32"))]
    laufzeit: Option<ScriptLaufzeit>,
    konsole: Vec<Zeile>,
    kamera: Kamera2d,
    kamera_vorher: Kamera2d,
    auswahl: Option<Objekt>,
    /// Beim Ziehen: Abstand Maus → Objektmitte
    ziehen: Option<Vec2>,
    /// Wo die Szene gezeichnet wird (Pixel); `None`, wenn ein Script-Tab offen ist
    bereich: Option<Bereich>,
    maus_welt: Vec2,
    sprites: Vec<Sprite>,
    statistik: FrameStatistik,
    fps: Fps,
}

impl Studio {
    pub fn new(renderer: &mut Renderer) -> Self {
        let mut assets = Assets::default();
        for t in TEXTUREN {
            assets
                .textur_laden(renderer, t)
                .expect("eingebaute Textur kaputt");
        }

        #[cfg(not(target_arch = "wasm32"))]
        let ordner = projekt::ordner_aus_argumenten();
        #[cfg(not(target_arch = "wasm32"))]
        let geladen = projekt::laden(&ordner);
        #[cfg(target_arch = "wasm32")]
        let geladen: Result<ProjektInhalt, String> = Ok(projekt::eingebaut());

        let mut studio = Self {
            projekt_name: String::new(),
            #[cfg(not(target_arch = "wasm32"))]
            ordner,
            welt: Welt::new(),
            assets,
            szene_id: None,
            scripts: Vec::new(),
            tab: Tab::Szene,
            modus: Modus::Bearbeiten,
            schnappschuss: None,
            kamera_vor_start: Kamera2d::default(),
            #[cfg(not(target_arch = "wasm32"))]
            motor: motor_im_hintergrund(),
            #[cfg(not(target_arch = "wasm32"))]
            laufzeit: None,
            konsole: Vec::new(),
            kamera: Kamera2d::default(),
            kamera_vorher: Kamera2d::default(),
            auswahl: None,
            ziehen: None,
            bereich: None,
            maus_welt: Vec2::ZERO,
            sprites: Vec::new(),
            statistik: FrameStatistik::default(),
            fps: Fps::default(),
        };

        match geladen {
            Ok(inhalt) => studio.projekt_uebernehmen(inhalt),
            Err(e) => studio.melden(Art::Fehler, e),
        }
        studio
    }

    fn projekt_uebernehmen(&mut self, inhalt: ProjektInhalt) {
        self.projekt_name = inhalt.name;
        self.scripts = inhalt
            .scripts
            .into_iter()
            .map(|s| ScriptTab {
                name: s.name,
                gespeichert: s.text.clone(),
                text: s.text,
            })
            .collect();
        if let Some(s) = inhalt.szene {
            self.szene_id = Some(s.id);
            match szene::laden(&mut self.welt, &self.assets, &s) {
                Ok(n) => self.melden(
                    Art::System,
                    format!(
                        "Projekt '{}' geladen: {n} Objekte, {} Scripts",
                        self.projekt_name,
                        self.scripts.len()
                    ),
                ),
                Err(e) => self.melden(Art::Fehler, e),
            }
        }
    }

    pub fn titel(&self) -> String {
        format!("Timbre Studio – {}", self.projekt_name)
    }

    fn melden(&mut self, art: Art, text: impl Into<String>) {
        self.konsole.push(Zeile {
            art,
            text: text.into(),
        });
        if self.konsole.len() > MAX_KONSOLE {
            let zu_viel = self.konsole.len() - MAX_KONSOLE;
            self.konsole.drain(..zu_viel);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn meldungen_uebernehmen(&mut self) {
        let Some(lz) = self.laufzeit.as_mut() else {
            return;
        };
        let meldungen = std::mem::take(&mut lz.kontext().meldungen);
        for m in meldungen {
            match m {
                Meldung::Info(t) => self.melden(Art::Info, t),
                Meldung::Fehler(t) => self.melden(Art::Fehler, t),
            }
        }
    }

    // -----------------------------------------------------------------------
    // ▶ / ■
    // -----------------------------------------------------------------------

    fn starten(&mut self) {
        if self.modus == Modus::Laeuft {
            return;
        }
        self.scripts_speichern();
        let id = self.szene_id.unwrap_or_default();
        let schnappschuss = match szene::speichern(&self.welt, &self.assets, id, &self.projekt_name)
        {
            Ok(s) => s,
            Err(e) => return self.melden(Art::Fehler, format!("Start nicht möglich: {e}")),
        };
        self.schnappschuss = Some(schnappschuss);
        self.kamera_vor_start = self.kamera;
        self.modus = Modus::Laeuft;
        self.tab = Tab::Szene;
        self.ziehen = None;
        self.melden(Art::System, "▶ Gestartet");

        #[cfg(not(target_arch = "wasm32"))]
        {
            let laufzeit = match self.motor_holen() {
                Ok(m) => m.starten(),
                Err(e) => Err(e),
            };
            match laufzeit {
                Ok(mut lz) => {
                    lz.kontext().texturen = self.assets.textur_namen().clone();
                    for s in &self.scripts {
                        lz.script_laden(&mut self.welt, &s.name, &s.text);
                    }
                    lz.start(&mut self.welt);
                    self.laufzeit = Some(lz);
                    self.meldungen_uebernehmen();
                }
                Err(e) => self.melden(Art::Fehler, format!("Scripting nicht verfügbar: {e}")),
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.melden(
            Art::System,
            "Scripts laufen im Browser erst ab Meilenstein 8.",
        );
    }

    fn stoppen(&mut self) {
        if self.modus != Modus::Laeuft {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.laufzeit = None;
        }
        if let Some(s) = self.schnappschuss.take() {
            let mut welt = Welt::new();
            match szene::laden(&mut welt, &self.assets, &s) {
                Ok(_) => self.welt = welt,
                Err(e) => self.melden(Art::Fehler, format!("Zurücksetzen fehlgeschlagen: {e}")),
            }
        }
        self.kamera = self.kamera_vor_start;
        self.kamera_vorher = self.kamera;
        self.auswahl = None;
        self.modus = Modus::Bearbeiten;
        self.melden(Art::System, "■ Gestoppt – Szene zurückgesetzt");
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn motor_holen(&mut self) -> Result<&ScriptMotor, String> {
        if let Motor::Laedt(rx) = &self.motor {
            // Notfalls warten, bis das Übersetzen fertig ist (dauert nur beim ersten Mal).
            self.motor = match rx.recv() {
                Ok(Ok(m)) => Motor::Bereit(m),
                Ok(Err(e)) => Motor::Kaputt(e),
                Err(_) => Motor::Kaputt("Übersetzen abgebrochen".into()),
            };
        }
        match &self.motor {
            Motor::Bereit(m) => Ok(m),
            Motor::Kaputt(e) => Err(e.clone()),
            Motor::Laedt(_) => unreachable!(),
        }
    }

    // -----------------------------------------------------------------------
    // Speichern
    // -----------------------------------------------------------------------

    fn scripts_speichern(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        for i in 0..self.scripts.len() {
            if self.scripts[i].text == self.scripts[i].gespeichert {
                continue;
            }
            match projekt::script_speichern(
                &self.ordner,
                &self.scripts[i].name,
                &self.scripts[i].text,
            ) {
                Ok(()) => self.scripts[i].gespeichert = self.scripts[i].text.clone(),
                Err(e) => self.melden(Art::Fehler, format!("Speichern fehlgeschlagen: {e}")),
            }
        }
    }

    fn alles_speichern(&mut self) {
        self.scripts_speichern();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.modus == Modus::Laeuft {
                return self.melden(
                    Art::System,
                    "Scripts gespeichert. Die Szene wird nur im Bearbeiten-Modus gespeichert.",
                );
            }
            let id = *self.szene_id.get_or_insert_with(AssetId::neu);
            let ergebnis = szene::speichern(&self.welt, &self.assets, id, &self.projekt_name)
                .and_then(|s| projekt::szene_speichern(&self.ordner, &s));
            match ergebnis {
                Ok(()) => self.melden(
                    Art::System,
                    format!(
                        "Gespeichert ({} Objekte, {} Scripts)",
                        self.welt.len(),
                        self.scripts.len()
                    ),
                ),
                Err(e) => self.melden(Art::Fehler, format!("Speichern fehlgeschlagen: {e}")),
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.melden(
            Art::System,
            "Im Browser kann (noch) nicht gespeichert werden.",
        );
    }

    // -----------------------------------------------------------------------
    // Logik-Schritt (60× pro Sekunde)
    // -----------------------------------------------------------------------

    pub fn schritt(&mut self, dt: f32, input: &Input, tastatur_frei: bool) {
        self.kamera_vorher = self.kamera;
        welt::schritt_beginnen(&mut self.welt);

        match self.modus {
            Modus::Bearbeiten => {
                if tastatur_frei {
                    let mut r = Vec2::ZERO;
                    if input.taste(KeyCode::KeyW) {
                        r.y += 1.0;
                    }
                    if input.taste(KeyCode::KeyS) {
                        r.y -= 1.0;
                    }
                    if input.taste(KeyCode::KeyA) {
                        r.x -= 1.0;
                    }
                    if input.taste(KeyCode::KeyD) {
                        r.x += 1.0;
                    }
                    self.kamera.position += r.normalize_or_zero() * 700.0 / self.kamera.zoom * dt;
                    if input.taste_neu(KeyCode::Delete) {
                        self.auswahl_loeschen();
                    }
                }
            }
            Modus::Laeuft => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(lz) = self.laufzeit.as_mut() {
                    if !lz.abgestuerzt() {
                        let k = lz.kontext();
                        k.input = if tastatur_frei {
                            input.clone()
                        } else {
                            Input::default()
                        };
                        k.maus_welt = self.maus_welt;
                        lz.update(&mut self.welt, dt);
                        if let Some(kamera) = lz.kontext().kamera.take() {
                            self.kamera = kamera;
                        }
                        let abgestuerzt = lz.abgestuerzt();
                        self.meldungen_uebernehmen();
                        if abgestuerzt {
                            self.melden(
                                Art::System,
                                "Scripts angehalten. Mit ■ stoppen, Fehler beheben, erneut ▶.",
                            );
                        }
                    }
                }
                welt::bewegen(&mut self.welt, dt);
            }
        }
    }

    pub fn bereich(&self) -> Option<Bereich> {
        self.bereich
    }

    pub fn sprites_bauen(&mut self, alpha: f32) -> (Kamera2d, &[Sprite]) {
        welt::sprites_sammeln(&self.welt, alpha, &mut self.sprites);
        (self.kamera_vorher.lerp(&self.kamera, alpha), &self.sprites)
    }

    pub fn statistik_setzen(&mut self, s: FrameStatistik, schritte: u32) {
        self.statistik = s;
        self.fps.frame(schritte);
    }

    // -----------------------------------------------------------------------
    // Objekte
    // -----------------------------------------------------------------------

    /// Oberstes Objekt unter einem Weltpunkt.
    fn treffer(&self, punkt: Vec2) -> Option<Objekt> {
        let mut bester: Option<(i32, Objekt)> = None;
        for (o, t, b) in self.welt.query::<(Objekt, &Transform, &Bild)>().iter() {
            let lokal = Vec2::from_angle(-t.drehung).rotate(punkt - t.position);
            let halb = b.groesse * t.skala * 0.5;
            if lokal.x.abs() <= halb.x.abs()
                && lokal.y.abs() <= halb.y.abs()
                && bester.is_none_or(|(e, _)| b.ebene >= e)
            {
                bester = Some((b.ebene, o));
            }
        }
        bester.map(|(_, o)| o)
    }

    fn objekt_anlegen(&mut self, name: &str, textur_name: &str, groesse: f32) {
        let Some(&textur) = self.assets.textur_namen().get(textur_name) else {
            return;
        };
        let o = self.welt.spawn((
            Name(name.into()),
            Transform::bei(self.kamera.position),
            Bild::new(textur, Vec2::splat(groesse)),
        ));
        self.auswahl = Some(o);
    }

    fn auswahl_loeschen(&mut self) {
        if let Some(o) = self.auswahl.take() {
            let _ = self.welt.despawn(o);
        }
    }

    // -----------------------------------------------------------------------
    // Oberfläche
    // -----------------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F5)) {
            if self.modus == Modus::Laeuft {
                self.stoppen()
            } else {
                self.starten()
            }
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::S)) {
            self.alles_speichern();
        }
        // Gelöschte Objekte (z. B. durch Scripts) nicht mehr auswählen
        if self.auswahl.is_some_and(|o| !self.welt.contains(o)) {
            self.auswahl = None;
        }

        egui::Panel::top("leiste").show(ui, |ui| self.leiste_ui(ui));
        egui::Panel::bottom("konsole")
            .resizable(true)
            .default_size(170.0)
            .show(ui, |ui| self.konsole_ui(ui));
        egui::Panel::left("objekte")
            .resizable(true)
            .default_size(210.0)
            .show(ui, |ui| self.objekte_ui(ui));
        egui::Panel::right("eigenschaften")
            .resizable(true)
            .default_size(250.0)
            .show(ui, |ui| self.eigenschaften_ui(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.mitte_ui(ui));
    }

    fn leiste_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("🌲 Timbre Studio").strong());
            ui.label(RichText::new(&self.projekt_name).weak());
            ui.separator();
            match self.modus {
                Modus::Bearbeiten => {
                    let knopf =
                        egui::Button::new(RichText::new("▶ Ausführen").color(Color32::WHITE))
                            .fill(Color32::from_rgb(0x3E, 0x7A, 0x3A));
                    if ui
                        .add(knopf)
                        .on_hover_text("Scripts starten (F5)")
                        .clicked()
                    {
                        self.starten();
                    }
                }
                Modus::Laeuft => {
                    let knopf = egui::Button::new(RichText::new("■ Stopp").color(Color32::WHITE))
                        .fill(Color32::from_rgb(0xA8, 0x3A, 0x2E));
                    if ui
                        .add(knopf)
                        .on_hover_text("Anhalten und Szene zurücksetzen (F5)")
                        .clicked()
                    {
                        self.stoppen();
                    }
                }
            }
            if ui
                .button("💾 Speichern")
                .on_hover_text("Scripts und Szene speichern (Strg+S)")
                .clicked()
            {
                self.alles_speichern();
            }
            ui.separator();
            let (text, farbe) = match self.modus {
                Modus::Bearbeiten => ("Bearbeiten", ui.visuals().weak_text_color()),
                Modus::Laeuft => ("Läuft", Color32::from_rgb(0x7C, 0xC4, 0x6A)),
            };
            ui.label(RichText::new(text).color(farbe));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "{:.0} FPS · {} Sprites · {} Draw Calls",
                        self.fps.wert, self.statistik.sprites, self.statistik.draw_calls
                    ))
                    .weak()
                    .monospace(),
                );
            });
        });
    }

    fn konsole_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Konsole").strong());
            if ui.small_button("Leeren").clicked() {
                self.konsole.clear();
            }
        });
        ui.separator();
        let fehler = Color32::from_rgb(0xE5, 0x6B, 0x5D);
        let system = ui.visuals().weak_text_color();
        let normal = ui.visuals().text_color();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for z in &self.konsole {
                    let (vorne, farbe) = match z.art {
                        Art::Info => ("", normal),
                        Art::System => ("· ", system),
                        Art::Fehler => ("✖ ", fehler),
                    };
                    ui.label(
                        RichText::new(format!("{vorne}{}", z.text))
                            .monospace()
                            .color(farbe),
                    );
                }
            });
    }

    fn objekte_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Objekte").strong());
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("+ Baumscheibe").clicked() {
                self.objekt_anlegen("Baumscheibe", "jahresringe", 120.0);
            }
            if ui.small_button("+ Blatt").clicked() {
                self.objekt_anlegen("Blatt", "blatt", 64.0);
            }
        });
        ui.separator();

        let mut liste: Vec<(Objekt, String)> = self
            .welt
            .query::<(Objekt, Option<&Name>)>()
            .iter()
            .map(|(o, n)| {
                (
                    o,
                    n.map(|n| n.0.clone())
                        .unwrap_or_else(|| format!("Objekt {}", o.id())),
                )
            })
            .collect();
        liste.sort_by(|a, b| natuerlich(&a.1, &b.1).then(a.0.id().cmp(&b.0.id())));

        let hoehe = ui.text_style_height(&egui::TextStyle::Button) + ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, hoehe, liste.len(), |ui, zeilen| {
                for (o, name) in &liste[zeilen] {
                    if ui
                        .selectable_label(self.auswahl == Some(*o), name)
                        .clicked()
                    {
                        self.auswahl = Some(*o);
                        if let Ok(t) = self.welt.get::<&Transform>(*o) {
                            self.kamera.position = t.position;
                            self.kamera_vorher = self.kamera;
                        }
                    }
                }
            });
    }

    fn eigenschaften_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Eigenschaften").strong());
        ui.separator();
        let Some(o) = self.auswahl else {
            ui.label(RichText::new("Klicke ein Objekt in der Szene oder in der Liste an.").weak());
            return;
        };
        if self.modus == Modus::Laeuft {
            ui.label(
                RichText::new("Läuft: Änderungen gehen bei ■ verloren.")
                    .weak()
                    .italics(),
            );
        }

        // Name
        let mut name = self
            .welt
            .get::<&Name>(o)
            .map(|n| n.0.clone())
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Name");
            if ui.text_edit_singleline(&mut name).changed() {
                let _ = self.welt.insert_one(o, Name(name.clone()));
            }
        });

        // Transform
        if let Ok(mut t) = self.welt.get::<&Transform>(o).map(|t| *t) {
            ui.add_space(4.0);
            ui.label(RichText::new("Transform").strong());
            let mut grad = t.drehung.to_degrees();
            let vorher = (t, grad);
            egui::Grid::new("transform").num_columns(2).show(ui, |ui| {
                ui.label("Position");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut t.position.x)
                            .speed(1.0)
                            .prefix("x "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut t.position.y)
                            .speed(1.0)
                            .prefix("y "),
                    );
                });
                ui.end_row();
                ui.label("Drehung");
                ui.add(egui::DragValue::new(&mut grad).speed(1.0).suffix("°"));
                ui.end_row();
                ui.label("Skala");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut t.skala.x)
                            .speed(0.01)
                            .prefix("x "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut t.skala.y)
                            .speed(0.01)
                            .prefix("y "),
                    );
                });
                ui.end_row();
            });
            if (t, grad) != vorher {
                t.drehung = grad.to_radians();
                let _ = self.welt.insert_one(o, t);
            }
        }

        // Bild
        if let Ok(mut b) = self.welt.get::<&Bild>(o).map(|b| *b) {
            ui.add_space(4.0);
            ui.label(RichText::new("Bild").strong());
            let vorher = b;
            let mut namen: Vec<(String, TexturId)> = self
                .assets
                .textur_namen()
                .iter()
                .map(|(n, t)| (n.clone(), *t))
                .collect();
            namen.sort();
            egui::Grid::new("bild").num_columns(2).show(ui, |ui| {
                ui.label("Textur");
                egui::ComboBox::from_id_salt("textur")
                    .selected_text(self.assets.name_von(b.textur).unwrap_or("?"))
                    .show_ui(ui, |ui| {
                        for (n, t) in &namen {
                            ui.selectable_value(&mut b.textur, *t, n);
                        }
                    });
                ui.end_row();
                ui.label("Größe");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::DragValue::new(&mut b.groesse.x)
                            .speed(1.0)
                            .prefix("b "),
                    );
                    ui.add(
                        egui::DragValue::new(&mut b.groesse.y)
                            .speed(1.0)
                            .prefix("h "),
                    );
                });
                ui.end_row();
                ui.label("Farbe");
                ui.color_edit_button_rgba_unmultiplied(&mut b.farbe);
                ui.end_row();
                ui.label("Ebene");
                ui.add(egui::DragValue::new(&mut b.ebene).speed(0.1));
                ui.end_row();
            });
            if b != vorher {
                let _ = self.welt.insert_one(o, b);
            }
        }

        // Bewegung
        ui.add_space(4.0);
        match self.welt.get::<&Bewegung>(o).map(|b| *b) {
            Ok(mut bew) => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Bewegung").strong());
                    if ui.small_button("entfernen").clicked() {
                        let _ = self.welt.remove_one::<Bewegung>(o);
                    }
                });
                let vorher = bew;
                egui::Grid::new("bewegung").num_columns(2).show(ui, |ui| {
                    ui.label("Tempo");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::DragValue::new(&mut bew.linear.x)
                                .speed(1.0)
                                .prefix("x "),
                        );
                        ui.add(
                            egui::DragValue::new(&mut bew.linear.y)
                                .speed(1.0)
                                .prefix("y "),
                        );
                    });
                    ui.end_row();
                    ui.label("Drehen");
                    ui.add(
                        egui::DragValue::new(&mut bew.drehung)
                            .speed(0.05)
                            .suffix(" rad/s"),
                    );
                    ui.end_row();
                });
                if bew != vorher && self.welt.contains(o) {
                    let _ = self.welt.insert_one(o, bew);
                }
            }
            Err(_) => {
                if ui
                    .small_button("+ Bewegung")
                    .on_hover_text("Bewegt sich beim Ausführen von selbst")
                    .clicked()
                {
                    let _ = self.welt.insert_one(o, Bewegung::default());
                }
            }
        }

        ui.add_space(12.0);
        if ui.button("🗑 Löschen").on_hover_text("Entf").clicked() {
            self.auswahl_loeschen();
        }
    }

    fn mitte_ui(&mut self, ui: &mut egui::Ui) {
        // Reiter: Szene + ein Reiter pro Script
        egui::Frame::new()
            .fill(ui.visuals().panel_fill)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .selectable_label(self.tab == Tab::Szene, "🎬 Szene")
                        .clicked()
                    {
                        self.tab = Tab::Szene;
                    }
                    for (i, s) in self.scripts.iter().enumerate() {
                        let titel = if s.text != s.gespeichert {
                            format!("📜 {} •", s.name)
                        } else {
                            format!("📜 {}", s.name)
                        };
                        if ui
                            .selectable_label(self.tab == Tab::Script(i), titel)
                            .clicked()
                        {
                            self.tab = Tab::Script(i);
                        }
                    }
                    if ui.small_button("+ Script").clicked() {
                        let mut n = 1;
                        while self
                            .scripts
                            .iter()
                            .any(|s| s.name == format!("script{n}.luau"))
                        {
                            n += 1;
                        }
                        self.scripts.push(ScriptTab {
                            name: format!("script{n}.luau"),
                            text: NEUES_SCRIPT.into(),
                            gespeichert: String::new(),
                        });
                        self.tab = Tab::Script(self.scripts.len() - 1);
                    }
                });
            });

        match self.tab {
            Tab::Szene => self.szene_ui(ui),
            Tab::Script(i) => {
                self.bereich = None;
                self.editor_ui(ui, i);
            }
        }
    }

    fn editor_ui(&mut self, ui: &mut egui::Ui, i: usize) {
        let Some(script) = self.scripts.get_mut(i) else {
            self.tab = Tab::Szene;
            return;
        };
        egui::Frame::new()
            .fill(ui.visuals().extreme_bg_color)
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut layouter =
                            |ui: &egui::Ui, text: &dyn egui::TextBuffer, breite: f32| {
                                luau_farben::layout(ui, text.as_str(), breite)
                            };
                        ui.add(
                            egui::TextEdit::multiline(&mut script.text)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(40)
                                .frame(egui::Frame::NONE)
                                .layouter(&mut layouter),
                        );
                    });
            });
    }

    fn szene_ui(&mut self, ui: &mut egui::Ui) {
        let (rect, antwort) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        self.bereich = Some(Bereich {
            x: rect.min.x * ppp,
            y: rect.min.y * ppp,
            breite: rect.width() * ppp,
            hoehe: rect.height() * ppp,
        });
        let groesse = vec2(rect.width() * ppp, rect.height() * ppp);
        let zu_pixel = |p: egui::Pos2| vec2((p.x - rect.min.x) * ppp, (p.y - rect.min.y) * ppp);

        if let Some(p) = ui.input(|i| i.pointer.latest_pos()) {
            self.maus_welt = self.kamera.bildschirm_zu_welt(zu_pixel(p), groesse);
        }

        // Zoomen mit dem Mausrad (Punkt unter der Maus bleibt stehen)
        if antwort.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let vorher = self.maus_welt;
                self.kamera.zoom = (self.kamera.zoom * 1.0015f32.powf(scroll)).clamp(0.05, 20.0);
                if let Some(p) = antwort.hover_pos() {
                    let nachher = self.kamera.bildschirm_zu_welt(zu_pixel(p), groesse);
                    self.kamera.position += vorher - nachher;
                }
                self.kamera_vorher = self.kamera;
            }
        }

        // Schieben mit rechter oder mittlerer Maustaste
        if antwort.dragged_by(PointerButton::Secondary) || antwort.dragged_by(PointerButton::Middle)
        {
            let d = antwort.drag_delta() * ppp;
            self.kamera.position += vec2(-d.x, d.y) / self.kamera.zoom;
            self.kamera_vorher = self.kamera;
        }

        // Auswählen und Verschieben mit der linken Maustaste (nur im Bearbeiten-Modus)
        if self.modus == Modus::Bearbeiten {
            let (gedrueckt, unten) =
                ui.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down()));
            if gedrueckt && antwort.hovered() {
                self.auswahl = self.treffer(self.maus_welt);
                self.ziehen = self.auswahl.and_then(|o| {
                    self.welt
                        .get::<&Transform>(o)
                        .ok()
                        .map(|t| t.position - self.maus_welt)
                });
            }
            if !unten {
                self.ziehen = None;
            }
            if let (Some(o), Some(versatz)) = (self.auswahl, self.ziehen) {
                if let Ok(mut t) = self.welt.get::<&mut Transform>(o) {
                    t.position = self.maus_welt + versatz;
                }
            }
        }

        // Rahmen um das ausgewählte Objekt
        if let Some(o) = self.auswahl {
            if let Ok((t, b)) = self
                .welt
                .query_one::<(&Transform, &Bild)>(o)
                .get()
                .map(|(t, b)| (*t, *b))
            {
                let mitte = self.kamera.welt_zu_bildschirm(t.position, groesse) / ppp;
                let halb = (b.groesse * t.skala).abs() * 0.5 * self.kamera.zoom / ppp;
                let r = egui::Rect::from_center_size(
                    egui::pos2(rect.min.x + mitte.x, rect.min.y + mitte.y),
                    egui::vec2(halb.x * 2.0, halb.y * 2.0),
                );
                ui.painter().with_clip_rect(rect).rect_stroke(
                    r.expand(3.0),
                    2.0,
                    Stroke::new(1.5, Color32::from_rgb(0xF2, 0xC1, 0x4E)),
                    egui::StrokeKind::Outside,
                );
            }
        }

        // Hinweis unten links
        let hinweis = match self.modus {
            Modus::Bearbeiten => "Links: auswählen/verschieben · Rechts: Ansicht schieben · Rad: zoomen · WASD: Ansicht",
            Modus::Laeuft => "Läuft – F5 oder ■ Stopp beendet und setzt die Szene zurück",
        };
        ui.painter().text(
            rect.left_bottom() + egui::vec2(8.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            hinweis,
            egui::FontId::proportional(12.0),
            Color32::from_white_alpha(110),
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn motor_im_hintergrund() -> Motor {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(ScriptMotor::neu(LUAU_GAST));
    });
    Motor::Laedt(rx)
}

/// Vergleich, bei dem Zahlen als Zahlen zählen: „Baumstumpf 2“ vor „Baumstumpf 10“.
fn natuerlich(a: &str, b: &str) -> std::cmp::Ordering {
    let teile = |s: &str| -> Vec<(String, u64)> {
        let mut v = Vec::new();
        let mut text = String::new();
        let mut zahl = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() {
                zahl.push(c);
            } else {
                if !zahl.is_empty() {
                    v.push((std::mem::take(&mut text), zahl.parse().unwrap_or(0)));
                    zahl.clear();
                }
                text.extend(c.to_lowercase());
            }
        }
        v.push((text, zahl.parse().unwrap_or(0)));
        v
    };
    teile(a).cmp(&teile(b))
}

/// Bilder pro Sekunde, einmal pro Sekunde aktualisiert.
struct Fps {
    wert: f32,
    frames: u32,
    seit: web_time::Instant,
}

impl Default for Fps {
    fn default() -> Self {
        Self {
            wert: 0.0,
            frames: 0,
            seit: web_time::Instant::now(),
        }
    }
}

impl Fps {
    fn frame(&mut self, _schritte: u32) {
        self.frames += 1;
        let t = self.seit.elapsed().as_secs_f32();
        if t >= 1.0 {
            self.wert = self.frames as f32 / t;
            self.frames = 0;
            self.seit = web_time::Instant::now();
        }
    }
}
