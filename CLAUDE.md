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
  - Lua / Luau: Interpreter als WASM-Gast (`assets/scripting/luau.wasm` bzw. `lua.wasm`, gebaut mit `tools/luau-gast/bauen.sh` bzw. `tools/lua-gast/bauen.sh` via zig; nutzen WebAssembly-Exceptions/exnref), laden Script-Quelltext zur Laufzeit. Sprache = Dateiendung (`.luau` / `.lua`, `projekt::Sprache`).
  - **Mehrere Sprachen gleichzeitig:** pro Sprache ein `ScriptMotor` (Hintergrund-Übersetzung) und beim ▶ eine `ScriptLaufzeit` (`Studio::laufzeiten`). Physik einmal pro Schritt, `beruehrungen_melden` an jede Laufzeit. `broadcast` geht über den Host auch an andere Sprachen (Import `nachricht` → `Kontext::ausgehend` → `Studio::nachrichten_verteilen` → Export `timbre_nachricht`; nur einfache Daten, max. 8 Runden pro Schritt). `shared` bleibt pro Sprache.
  - Script-API ist englisch (`timbre.spawn`, `timbre.set_position` …), Engine-Code deutsch. Doku für Nutzer: `docs/scripting.md`.
  - **Python (geändert, begründet):** statt componentize-py **CPython 3.12 selbst als Core-Modul-Gast** mit derselben ABI (`tools/python-gast`: `gast.c` + `laufzeit.py`). Grund: componentize-py braucht das Component Model, das wir in M5 verlassen haben; so ist es echtes Python mit Standardbibliothek. Paddy: „mach wie es am besten passt“. Standardbibliothek als Zip im schreibgeschützten Speicher-Dateisystem des Hosts (`wasi.rs`). Warten = `async def` + `await task.wait(s)`. Gast wird nur bei Bedarf übersetzt und im Temp-Ordner zwischengespeichert.
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
- **Projekt-Assets:** Texturen liegen als `.tbr` in `<projekt>/texturen/` (PNG per Drag & Drop → wird dort angelegt), Sounds als normale Dateien (WAV/OGG/MP3/FLAC) in `<projekt>/sounds/`. Sounds bekommen **kein** `.tbr`-Format (kein Mehrwert, Audiodateien sind schon Standard); Scripts spielen sie per Name ohne Endung. Audio nativ über **rodio 0.23** (`timbre-core/src/audio.rs`, kein Audiogerät = stumm, kein Fehler); Browser später WebAudio (M8).
- **Assets:** eigenes Format `.tbr` (Mesh, Textur, Szene). Assets liegen lokal bzw. später auf Paddys Homeserver – **nicht** in Supabase. Supabase höchstens für Metadaten/Login (Grove-Auth).

## Aufbau des Repos
- `crates/timbre-tbr` – `.tbr` lesen/schreiben, **ohne Abhängigkeiten** (auch für Werkzeuge, Grove, Bridge). Spezifikation: `docs/tbr-format.md`.
- `crates/timbre-core` – Engine: Renderer, Zeit, Input, Kamera, ECS-Welt (`welt.rs`), Assets (`assets.rs`), Szenen (`szene.rs`); später Scripting-Host.
- `crates/timbre-app` – **Timbre Studio** (Editor): `studio.rs` (Modi, Panels, Ausführen/Stopp), `projekt.rs` (Projektordner), `oberflaeche.rs` (egui ↔ winit/wgpu), `luau_farben.rs` (Syntaxfarben), `vervollstaendigung.rs` (Autovervollständigung).
- `projekte/beispiel` – Beispielprojekt: `szene.tbr` + `scripts/*.luau` + `sounds/*.wav` (+ `texturen/*.tbr`, sobald etwas importiert wird). Studio startet standardmäßig damit (`cargo run -p timbre-app -- <ordner>` für andere).
- `tools/luau-gast` – Luau → WebAssembly (nur bei Luau-/Gast-Änderungen nötig, Ergebnis liegt im Repo).
- `tools/python-gast` – Python 3.12 → WebAssembly (lädt wasi-sdk 20 + libpython in `.cache/`, dünnt die Standardbibliothek aus). Script-API-Änderungen auch hier nachziehen (`gast.c` + `laufzeit.py`).
- `crates/timbre-core/src/wasi.rs` – WASI-Funktionen für alle Gäste, schreibgeschütztes Speicher-Dateisystem.
- `tools/lua-gast` – Lua 5.4 → WebAssembly, gleiche Logik wie der Luau-Gast (Änderungen an der Script-API immer in **beiden** `gast.cpp` nachziehen und beide neu bauen).
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
5. [~] WASM-Scripting-Host (wasmtime) + erste Sprache: **Luau**, dann **Lua** – dazu Timbre Studio (Editor) – alles gebaut, Pakete 2–4 + Lua warten auf Paddys Validierung
6. [~] **Python** (gebaut, wartet auf Validierung), dann **Java**
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
- Paket 3 „Bilder & Sounds“ gebaut (in Claudes Testumgebung geprüft: PNG + WAV importiert, gespeichert, neu geladen, Spiel mit Sounds gestartet; Ton selbst nur gegen ein Null-Audiogerät getestet; wartet zusammen mit Paket 2 auf Validierung):
  - Drag & Drop ins Studio-Fenster (egui `raw.dropped_files`, abgearbeitet in `Studio::importieren` nach der Oberfläche, weil Texturen den Renderer brauchen). Beim Ziehen dunkler Hinweis über dem Fenster.
  - PNG → `texturen/<name>.tbr` (Name = Dateiname, bei Gleichheit `name2` …), sofort geladen, Objekt mitten in der Ansicht (max. 256 px an der längsten Seite). Projekt-Texturen werden beim Start vor der Szene geladen.
  - Sounds → Kopie nach `sounds/`, einmal angespielt. Explorer: aufklappbare Bereiche **🖼 Bilder** (klicken = Objekt setzen) und **🔊 Sounds** (▶ anhören, ■ alle stoppen).
  - Script-API: `timbre.play_sound(name [, lautstärke [, schleife]])`, `timbre.stop_sounds([name])`. ABI: neue Importe `play_sound`, `stop_sounds` (WIT nachgezogen); Host sammelt Befehle im Kontext (`SoundBefehl`), Studio spielt sie in `meldungen_uebernehmen` ab; ■ stoppt alle Sounds. `luau.wasm` neu gebaut (zig 0.17 über pip `ziglang`). Test `sounds_werden_angefordert`.
  - `assets::png_lesen` (Kern, `png`-Crate), `Assets::pixel_groesse`.
  - Beispielprojekt: `sounds/Muenze.wav`, `sounds/Jubel.wav` (von Claude synthetisiert) – Sammelspiel spielt sie beim Einsammeln bzw. alle 5 Goldstücke.
- Paket 4 „Komfort beim Scripten“ gebaut (in Claudes Testumgebung geprüft: Vorschläge getippt/übernommen, Script während des Laufens von außen geändert → neu geladen; wartet auf Validierung):
  - **Live-Neuladen:** Strg+S im laufenden Spiel → `ScriptLaufzeit::script_neu_laden` → Gast-Export `timbre_neu_laden` (optional in der ABI). Alte Fassung (gleiche script_id) wird angehalten, neue geladen, `on_start` läuft sofort. Syntaxfehler → alte läuft weiter. Objekte der alten Fassung bleiben.
  - Gast merkt sich jetzt, welchem Script eine wartende Aufgabe gehört (`aktuell`, `Warten.script`): Aufgaben angehaltener Scripts enden (vorher liefen sie nach `timbre_script_stoppen` weiter – Bug behoben).
  - **Änderungen von außen** (VS Code): Studio vergleicht jede Sekunde die Script-Dateien mit dem gespeicherten Stand (`platte_abgleichen`). Unverändert im Studio → übernehmen (+ live neu laden); auch im Studio geändert → einmaliger Konflikt-Hinweis, Studio-Fassung gewinnt beim Speichern.
  - **Autovervollständigung** (`vervollstaendigung.rs`): nach `timbre.`/`task.`/`math.`/`string.`/`table.`/`script.` deren Funktionen mit Signatur; sonst Schlüsselwörter, Ereignisse, globale Funktionen, Namen aus dem Script. Nicht in Kommentaren/Strings. Tasten werden vor dem TextEdit per `consume_key` abgefangen; Funktionen bekommen `()` (Cursor in die Klammern). Strg+Leertaste öffnet von Hand. 4 Tests.
- **Lua 5.4** als zweite Sprache gebaut (in Claudes Testumgebung geprüft: `Zeit.lua` läuft neben den Luau-Scripts, Sprache eines Scripts per Eigenschaften umgestellt, gespeichert, wieder gestartet; wartet auf Validierung):
  - `tools/lua-gast/gast.cpp` = Luau-Gast mit Lua-5.4-API (`luaL_loadbuffer` + `_ENV`-Upvalue statt `luau_load`, `luaL_ref`, `lua_resume` mit 4 Argumenten, IDs als `lua_Integer`). Ohne io/os/package/debug, `dofile`/`loadfile`/`collectgarbage` entfernt. Lua-Quellen: Crate `lua-src` 548.1.1 (Lua 5.4.8), C mit `-D_WASI_EMULATED_SIGNAL`, Gast mit `-fno-exceptions`. 286 KB.
  - Host: zusätzliche WASI-Stubs (`fd_fdstat_set_flags`, `fd_read`, `fd_renumber`, `fd_prestat_*`, `path_open`, `proc_exit`), die Lua über stdio mitbringt. 3 Lua-Tests.
  - Studio: `Sprache`-Enum (Endung, Name, Vorlage), Explorer-Menüs „Script hinzufügen (Luau / Lua 5.4)“, Eigenschaften → Sprache wechseln (benennt Datei um), mehrere Laufzeiten, Live-Neuladen pro Sprache, Autovervollständigung ohne `continue`/`typeof` in Lua.
  - Beispielprojekt: `scripts/Zeit.lua` (Spielzeit-Anzeige) als Script-Objekt in `szene.tbr`.
  - Browser-Build nicht geprüft (wasm32-Target hier nicht installierbar); die neuen Teile sind sauber hinter `cfg(not(wasm32))`.
- Danach (Paddy unterwegs, „mach weiter“) gebaut, wartet ebenfalls auf Validierung:
  - **Autovervollständigung erweitert:** Vorschläge **in Texten** – Sounds (`play_sound("`), Bilder (`set_sprite(id, "`), Objekte (`find("`, Vergleiche `== "`), Tasten (`key_down("`), Maustasten, Kollisionsformen, Nachrichten (aus allen `broadcast("…")` im Projekt). Übernehmen ergänzt das schließende `"`. **Signatur-Hinweis** über dem Cursor, solange er in den Klammern einer bekannten Funktion steht (mit Argument-Nummer). `Projektnamen` baut das Studio nur, wenn der Editor Fokus hat. 7 Tests.
  - **`broadcast` über Sprachgrenzen** (siehe Architektur). Beide Gäste neu gebaut. WIT v0.5 (`simple-value`). Test `nachrichten_zwischen_sprachen`. Beispiel: `Zeit.lua` zeigt „letztes Goldstück bei …“ nach der Luau-Nachricht `gesammelt`.
- **Meilenstein 6, Schritt 1: Python 3** gebaut (Paddy: „mach wie es am besten passt“; in Claudes Testumgebung geprüft: `Funkeln.py` läuft neben Luau und Lua, empfängt `gesammelt` aus Luau, wartet mit `await task.wait`; wartet auf Validierung):
  - Gast: CPython 3.12.0 als `libpython3.12.a` (WASI, VMware Labs „WebAssembly Language Runtimes“) + `gast.c` (Modul `timbre` per C-API, Exporte wie Luau) + `laufzeit.py` (eingebettet als Text: eigene Globals pro Script, `task` mit async/await, Fehler als `Script:Zeile: Typ: Text`, Live-Neuladen, `broadcast` lokal + über den Host). Eigene `_PyImport_Inittab` mit nur nötigen C-Modulen → 11,4 MB statt 19,5 MB. Standardbibliothek ausgedünnt (ohne Netz/GUI/Tests/CJK) → `python312.zip` 2,1 MB.
  - Host: `wasi.rs` ersetzt die alten WASI-Stubs – alle preview1-Funktionen, Speicher-Dateisystem (fd 3 = `/`), stdout/stderr → Konsole, `random_get` per xorshift. `ScriptMotor::mit_dateien`. **Zwischenspeicher** für übersetzte Module (`%TEMP%\timbre-cache\*.cwasm`, Schlüssel = Hash aus Wasm + `precompile_compatibility_hash`) – zweiter Start sofort.
  - Studio: `Sprache::Python` (`.py`), Motor nur bei Bedarf (`motor_vorbereiten` beim Laden/Anlegen/Wechseln), Syntaxfarben für Python (`luau_farben::layout(…, python)`), Autovervollständigung mit Python-Schlüsselwörtern/Builtins, `math.`/`random.`/`task.` für Python, `#`-Kommentare. Tests: 2 Python-Laufzeit-Tests, 1 WASI-Test, 1 Autovervollständigungs-Test.
  - Beispielprojekt: `scripts/Funkeln.py` als Script-Objekt in `szene.tbr`.
  - Pushen von Claudes Umgebung aus geht nicht (Repo ist für die Sitzung nicht freigegeben) – Paddy committet/pusht selbst.
  - Nächste Schritte: Paddy validiert Meilenstein 5 + Python. Danach **Java** (Plan: TeaVM → WASM; prüfen, ob TeaVMs WASI-/GC-Ausgabe zur Core-Modul-ABI passt, sonst kleiner Java-Interpreter als Gast).

