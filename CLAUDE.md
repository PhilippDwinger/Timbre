# Timbre – Handover für Claude Code

Timbre ist Paddys eigene Engine (Rust). Der Name verbindet *Timbre* (Klangfarbe, Jazz) mit *timber* (Holz) und passt so zu Grove.
Repo: https://github.com/PhilippDwinger/Timbre

## Ziele
- **Timbre Studio** als Herzstück (Paddys Vorgabe): Bearbeiten-Modus wie Roblox Studio – Szene bearbeiten, Scripts schreiben, **▶ Ausführen** startet alles, **■ Stopp** setzt die Szene zurück.
- Engine, die nativ (Windows) UND im Browser (WebGPU, eingebettet in Grove) läuft – eine Codebasis.
- Scripting in vielen Sprachen. **Pflicht: Lua, Luau, Java, Python.** Danach so viele wie möglich.
- Eigene Editoren, gebaut AUF der Engine (nicht daneben):
  - **Timbre Carve** – Mesh-Editor
  - **Timbre Stain** – Mal- und Texture-Editor (später: Malen direkt aufs Mesh über UVs)
- Anbindung an **Grove** (Web) und **Bridge** (Paddys Python-Server mit Realms).

## Architektur-Entscheidungen (fix)
- **Sprache:** Rust, Edition 2021, Cargo-Workspace unter `crates/`.
- **Rendering:** wgpu (nativ: DX12/Vulkan, Web: WebGPU). Fenster nativ: winit.
- **Scripting:** Jede Sprache ist ein **WASM-Gast**.
  - **Geändert in M5 (begründet):** Gäste sind **Core-Module mit schmaler C-ABI** (`docs/scripting-abi.md`) statt Component Model – ein Host-Code für nativ und Browser, keine Adapter/jco nötig, winzige Schnittstelle. `wit/timbre.wit` bleibt die sprachneutrale Beschreibung derselben Funktionen und muss mit der ABI synchron bleiben.
  - Nativ: wasmtime (mit `wasm_exceptions` + Fuel gegen Endlosschleifen). Web: Browser-WebAssembly (M8).
  - Lua / Luau: Interpreter als WASM-Gast (`assets/scripting/luau.wasm`, gebaut mit `tools/luau-gast/bauen.sh` via zig; nutzt WebAssembly-Exceptions/exnref), lädt Script-Quelltext zur Laufzeit.
  - Script-API ist englisch (`timbre.spawn`, `timbre.set_position` …), Engine-Code deutsch. Doku für Nutzer: `docs/scripting.md`.
  - Python: componentize-py (groß → lazy laden).
  - Java: TeaVM → WASM; ggf. dünner Adapter nötig (Component-Model-Tooling für Java noch unreif).
  - Bonus-Sprachen: Rust, C, C++, C#, Go (TinyGo), Zig, AssemblyScript, JavaScript, Kotlin.
- **Zweite Steuer-Ebene:** Timbre wird ein **Bridge-Client** in einem Realm und bietet Funktionen an (Szene laden, Objekt spawnen, Mesh exportieren …). Asynchron, für Fernsteuerung aus jeder Sprache mit Bridge-Extension.
- **ECS:** `hecs` (klein, schnell, läuft im Browser). Objekte ↔ `u64` über `welt::objekt_nummer` / `objekt_aus_nummer` – passt zu `spawn → u64` in `wit/timbre.wit`.
- **Szenen-Inhalt:** JSON (ein Objekt pro Zeile) im `TEXT`-Block einer Szenen-`.tbr`; Texturen per Asset-ID. Lesbar auch für Grove (JS) und Bridge (Python).
- **Scripts sind Objekte** (Paddys Vorgabe, Roblox-Prinzip): Komponente `Script { datei, aktiv }` + optional `Eltern(Objekt)`. Quelltext bleibt Datei in `scripts/` (für Git/VS Code); Name ändern = Datei umbenennen (beim Speichern). Dateien ohne Script-Objekt wandern beim Speichern nach `scripts/.papierkorb/`. Ältere Projekte: lose `.luau`-Dateien werden beim Laden zu Script-Objekten.
- **Baum:** `Eltern`-Komponente, Löschen entfernt Nachkommen (`welt::entfernen`).
- **Transform ist relativ zum nächsten Vorfahren mit Transform** (Ordner/Scripts ohne Transform zählen nicht). `welt::welt_transform` / `welt_transform_setzen` rechnen um; `eltern_setzen` hängt um, ohne dass das Objekt springt. Script-API (`get/set_position`, `get/set_rotation`) arbeitet in **Weltkoordinaten** (wie Roblox `Position`), das Eigenschaften-Panel zeigt lokale Werte.
- **Kollisionen** (`physik.rs`): Kreis / ungedrehtes Rechteck, Größe aus Bild × Skala; `fest` (blockiert) vs. Auslöser; `verankert` (wie Anchored). Feste, nicht verankerte werden herausgeschoben, Geschwindigkeit in die Wand wird gekappt. O(n²) über Objekte mit Kollision – reicht für einige hundert. `on_touch` nur bei **neuer** Berührung, an Scripts, deren Eltern beteiligt sind.
- **Luau-Gast-Laufzeit:** `on_start`, `on_touch`, `on_message` laufen als Coroutinen (dürfen `task.wait`), `on_update` per pcall (kein Warten). Achtung im Gast-C++: immer auf dem Stack des übergebenen `lua_State` arbeiten (Coroutinen!), nicht auf dem globalen `L`.
- **Rückgängig** = Schnappschüsse des Szenen-JSON (max. 100), gemerkt nach abgeschlossener Änderung (`markieren()` an jeder Änderungsstelle im Studio aufrufen!).
- **Reihenfolge:** Paddy will erst die Engine verbessern, dann weitere Sprachen.
- **Assets:** eigenes Format `.tbr` (Mesh, Textur, Szene). Assets liegen lokal bzw. später auf Paddys Homeserver – **nicht** in Supabase. Supabase höchstens für Metadaten/Login (Grove-Auth).

## Aufbau des Repos
- `crates/timbre-tbr` – `.tbr` lesen/schreiben, **ohne Abhängigkeiten** (auch für Werkzeuge, Grove, Bridge). Spezifikation: `docs/tbr-format.md`.
- `crates/timbre-core` – Engine: Renderer, Zeit, Input, Kamera, ECS-Welt (`welt.rs`), Assets (`assets.rs`), Szenen (`szene.rs`); später Scripting-Host.
- `crates/timbre-app` – **Timbre Studio** (Editor): `studio.rs` (Modi, Panels, Ausführen/Stopp), `projekt.rs` (Projektordner), `oberflaeche.rs` (egui ↔ winit/wgpu), `luau_farben.rs` (Syntaxfarben).
- `projekte/beispiel` – Beispielprojekt: `szene.tbr` + `scripts/*.luau`. Studio startet standardmäßig damit (`cargo run -p timbre-app -- <ordner>` für andere).
- `tools/luau-gast` – Luau → WebAssembly (nur bei Luau-/Gast-Änderungen nötig, Ergebnis liegt im Repo).
- `crates/tbr` – Kommandozeilen-Werkzeug: `tbr info`, `tbr textur` (PNG), `tbr mesh` (glTF/GLB).
- `assets/quellen` – Ausgangsdateien (PNG, glTF); `assets/texturen`, später `assets/meshes` – fertige `.tbr`.

## Harte Regeln
- **Bridge niemals verändern.** Timbre spricht nur Bridges Protokoll als Client. Bridge-Anbindung erst, wenn Bridge 1.1 steht.
- Formatänderungen an `.tbr` immer in `docs/tbr-format.md` nachziehen; ab Formatversion 1 müssen alte Dateien lesbar bleiben.
- Schnittstellenänderungen an `wit/timbre.wit` immer begründen und in diesem Dokument nachziehen – alle Sprachen hängen daran.
- Arbeitsweise: kleine Schritte, Paddy validiert jeden Schritt live, bevor es weitergeht. Neue Konzepte kurz erklären.
- Windows, Befehle für CMD (nicht PowerShell). Befehle immer mit echten Werten, keine Platzhalter.
- Nach jedem abgeschlossenen Meilenstein: Status unten aktualisieren.

## Meilensteine
1. [x] Fenster + wgpu: farbiges Dreieck nativ, danach dasselbe im Browser (wasm32-unknown-unknown)
2. [x] Fixed-Timestep Game Loop, Input, Kamera, Sprite-Rendering mit Batching
3. [x] Asset-Format `.tbr` v0 (Mesh, Textur, Szene) – wichtigste Entscheidung, alles baut darauf auf
4. [x] ECS
5. [~] WASM-Scripting-Host (wasmtime) + erste Sprache: **Luau**, dann **Lua** – dazu Timbre Studio (Editor)
6. [ ] **Python**, dann **Java**
7. [ ] Bonus-Sprachen (Rust, C#, JS … zuerst die einfachen)
8. [ ] Web-Build in Grove einbetten
9. [ ] Timbre Carve (Mesh-Editor, Half-Edge-Datenstruktur)
10. [ ] Timbre Stain (Texture/Malen, GPU-Pinsel, Ebenen), danach Malen aufs Mesh
11. [ ] Bridge-Client (nach Bridge 1.1)

## Status
- Grundgerüst angelegt: Workspace, `timbre-core` (Bibliothek), `timbre-app` (Starter), WIT-Entwurf. Läuft bei Paddy.
- Meilenstein 1a (nativ) ✅ von Paddy validiert (Dreieck sichtbar, committet): winit 0.30 + wgpu 30. `timbre-core/src/renderer.rs` kennt kein Fenster-Crate (bekommt nur ein `SurfaceTarget`), damit er später auch im Browser läuft. Dreieck kommt ohne Vertex-Buffer direkt aus `shaders/dreieck.wgsl`.
  - wgpu-30-Eigenheiten: `InstanceDescriptor::new_without_display_handle()`, `get_current_texture()` liefert `CurrentSurfaceTexture` (kein Result), Präsentieren über `queue.present(frame)`.
- Meilenstein 1b (Browser) ✅ von Paddy validiert: Web-Build mit **Trunk** aus `crates/timbre-app` (`index.html` dort). Im Browser wird der Renderer asynchron gebaut (`spawn_local`) und kommt per `TimbreEvent::RendererBereit` über den `EventLoopProxy` zurück; Event-Loop dort über `spawn_app`. Logs/Panics in die Browser-Konsole. Braucht einen WebGPU-fähigen Browser (Chrome/Edge).
- Meilenstein 2 ✅ von Paddy validiert:
  - `zeit.rs` – `FesterTakt`: 60 Logik-Schritte/s, Akkumulator, max. 0,25 s nachholen, `alpha` für Interpolation beim Zeichnen. Nutzt `web-time` (std `Instant` stürzt im Browser ab).
  - `input.rs` – `Input`: gehalten / neu gedrückt (pro Logik-Schritt), Maus, Mausrad; Tasten über physische Position (`KeyCode::KeyW`).
  - `kamera.rs` – `Kamera2d`: Position + Zoom, 1 Welteinheit = 1 Pixel bei Zoom 1, Y nach oben. Ortho-Matrix von Hand (glam 0.34 hat die Projektions-API umgebaut).
  - `renderer.rs` – Sprite-Batching per Instancing: Sprites nach Textur stabil sortiert, ein Draw Call pro Textur-Gruppe, Instanz-Puffer wächst (Zweierpotenz). Alpha-Blending.
  - Folge der Sortierung: Zeichenreihenfolge zwischen Texturen ist noch nicht steuerbar → kommt mit Ebenen/Z-Wert (spätestens mit ECS).
  - Testszene `timbre-app/src/spiel.rs` (Steuerung siehe README), Texturen per Code (`texturen.rs`) bis `.tbr` existiert. Titelzeile zeigt FPS, Logik-Schritte/s, Sprites, Draw Calls.
  - Gemessen (Software-GPU): 12.580 Sprites = 2 Draw Calls.
- Meilenstein 3 ✅ von Paddy validiert: `.tbr`-Format v0 mit Paddy abgestimmt – ein Container für alles (Kopf + Blöcke, unbekannte Blöcke werden übersprungen), Meshes als Vielecke, Texturen mit optionalen Ebenen, Szenen als Text, Verweise über 128-Bit-Asset-IDs, eigenes Paket `timbre-tbr`, Werkzeug `tbr`.
  - Testszene lädt Texturen jetzt aus `assets/texturen/*.tbr` (per `include_bytes!`, damit es auch im Browser ohne Dateisystem geht). `tbr mesh` an einem glTF mit zwei transformierten Würfeln getestet.
  - Offen für M3: Meshes werden noch nicht gezeichnet (braucht 3D-Renderer), Szenen-Inhalt kommt mit ECS, Laden zur Laufzeit statt `include_bytes!`, optionale Kompression (Flag im Kopf ist reserviert).
- Meilenstein 4 ✅ von Paddy validiert:
  - `welt.rs`: Komponenten `Transform`, `Vorher` (automatisch, für Interpolation), `Bild` (mit `ebene` für die Zeichenreihenfolge), `Bewegung`, `Name`; Systeme `schritt_beginnen`, `bewegen`, `sprites_sammeln`.
  - Renderer sortiert jetzt nach (Ebene, Textur) → Ebenen-Problem aus M2 gelöst, Batching bleibt (ein Draw Call pro Ebene+Textur).
  - `assets.rs`: Asset-ID ↔ TexturId. `szene.rs`: Welt ↔ JSON; Laden prüft erst alles, bevor Objekte entstehen (kaputte Szene füllt die Welt nicht halb).
  - Testszene läuft komplett über die Welt; L = Ebenen tauschen, F5/F9 = Szene speichern/laden (`assets/szenen/gespeichert.tbr`, in .gitignore). `tbr text` zeigt Szeneninhalt.
  - Offen: Eltern/Kind-Hierarchie (Transform relativ zum Elternobjekt) – kommt, wenn der Editor oder 3D sie braucht.
- Meilenstein 5, Schritt 1 ✅ von Paddy validiert („schon sehr gut“):
  - Luau als WASM-Gast läuft in wasmtime: Syntax- und Laufzeitfehler werden gemeldet (Datei:Zeile), Fehler hält nur das betroffene Script an, Endlosschleifen werden per Fuel abgebrochen, jedes Script hat eigene Umgebung (Sandbox). 4 Tests in `scripting.rs`.
  - Timbre Studio mit egui 0.36 (passt zu wgpu 30/winit 0.30): Leiste (▶/■, Speichern, FPS), Objektliste, Eigenschaften (Name, Transform, Bild, Bewegung), Reiter Szene + Scripts mit Luau-Editor, Konsole. F5 = Ausführen/Stopp, Strg+S = Speichern, Entf = Löschen.
  - Ausführen: Scripts werden gespeichert, Szene als Schnappschuss (Szenen-JSON) gemerkt; Stopp lädt den Schnappschuss → alles wie vorher.
  - Renderer: `frame_beginnen` / `sprites_zeichnen(bereich)` / `frame_beenden` – Szene wird nur im Szenen-Bereich des Editors gezeichnet; egui zeichnet über eine Nicht-sRGB-Ansicht derselben Fläche (`view_linear`).
  - `[profile.dev.package."*"] opt-level = 2`: sonst übersetzt wasmtime das Luau-Modul im Debug-Build extrem langsam.
  - Alte Blätter-Testszene (`spiel.rs`) entfernt – ihre Ideen leben im Beispielprojekt als Luau-Scripts weiter.
  - Browser-Build des Studios nicht geprüft (Claude kann hier kein wasm32 bauen); Scripts im Browser kommen mit M8.
- Meilenstein 5, Schritt 2 ✅ von Paddy validiert: Scripts als Objekte.
  - Explorer als Baum (Scripts zuerst, natürlich sortiert, große Ordner zugeklappt), Doppelklick öffnet Script bzw. fährt Kamera zum Objekt, Rechtsklick-Menü (Script hinzufügen, Duplizieren, Löschen), Strg+D dupliziert samt Kindern.
  - Eigenschaften: Name, Eltern (Auswahlliste, Schleifen ausgeschlossen), Script-Bereich (Sprache, Datei, Aktiviert, Zeilen, Öffnen).
  - Script-Reiter mit × schließbar; Puffer nach Dateiname (überlebt ■ Stopp, weil sich Objekt-IDs beim Zurücksetzen ändern).
  - Szenen-JSON: `id` (nur bei Objekten mit Kindern), `eltern`, `script {datei, aktiv}`, `position` optional. Test `baum_und_scripts_hin_und_zurueck`.
  - ABI: `timbre_laden` bekommt `script_id`, `eltern_id`; neues Export `timbre_script_stoppen`; neue Importe `set_parent`, `get_parent`. In Luau: `script.name/id/parent`.
  - Beispielprojekt: `Steuerung` hängt unter `Spieler` und nutzt `script.parent`; `Laubregen` legt seine Blätter in einen Ordner `Laub` unter sich.
  - `rustfmt.toml` mit `max_width = 120`.
- Paket 1 „Editor-Grundlagen“ ✅ von Paddy freigegeben:
  - Rückgängig/Wiederholen (Strg+Z / Strg+Y / Strg+Umschalt+Z, Knöpfe ↶ ↷) für alle Szenen-Änderungen; Script-Puffer bleiben im Speicher, damit Rückgängig gelöschte/umbenannte Scripts zurückholen kann.
  - Drag & Drop im Explorer (auf Objekt = Kind, auf freie Fläche = oberste Ebene, Schleifen verweigert).
  - Kinder bewegen sich mit (Position, Drehung, Skala), auch über Ordner ohne Transform hinweg; interpoliert gezeichnet. Tests `kinder_bewegen_sich_mit`, `positionen_von_kindern_sind_weltkoordinaten`.
  - Fehler in der Konsole anklickbar → Script öffnet sich, Cursor + Scroll zur Zeile, Zeile rot hinterlegt (bis zur nächsten Änderung oder ▶). Editor hat Zeilennummern.
  - Ideen-Liste danach: Paket 2 (Kollisionen + on_touch, mehr Script-API: Maus, wait/Timer, Nachrichten, Bildschirmtext; Gitter/Einrasten, Dreh-/Skalier-Griffe), Paket 3 (PNG-Import per Drag & Drop, Sounds), Paket 4 (Live-Neuladen von Scripts, Autovervollständigung). Danach weitere Sprachen.
- Paket 2 „Echte Spiele“ gebaut (in Claudes Testumgebung geprüft: Spiel gespielt, 2 Goldstücke gesammelt; wartet auf Validierung):
  - Kollisionen + Eigenschaften-Bereich „Kollision“ (Form, Fest, Verankert) + grüne Umrisslinie am ausgewählten Objekt; Szenen-JSON `kollision {form, fest, verankert}`.
  - Script-API: `on_touch`, `on_message`, `timbre.broadcast`, `shared`, `task.wait/delay/spawn`, `get_name`, `set_collision`, `get_touching`, `mouse_down/pressed` (nur über dem Spielbereich), `ui_text/ui_clear` (Bildschirmtexte, im Studio über der Szene gezeichnet).
  - Editor: Gitter (Größe einstellbar) mit Einrasten (Alt = frei), gedrehter Auswahlrahmen, Griffe zum Skalieren (Ecken) und Drehen (Kreis), Einrasten 15° / 0,05.
  - Beispielprojekt: Sammelspiel (`Sammelspiel.luau` + `on_touch` in `Steuerung.luau`), Baumstümpfe blockieren, Spieler hat Kollision.
