# Timbre – Handover für Claude Code

Timbre ist Paddys eigene Engine (Rust). Der Name verbindet *Timbre* (Klangfarbe, Jazz) mit *timber* (Holz) und passt so zu Grove.
Repo: https://github.com/PhilippDwinger/Timbre

## Ziele
- Engine, die nativ (Windows) UND im Browser (WebGPU, eingebettet in Grove) läuft – eine Codebasis.
- Scripting in vielen Sprachen. **Pflicht: Lua, Luau, Java, Python.** Danach so viele wie möglich.
- Eigene Editoren, gebaut AUF der Engine (nicht daneben):
  - **Timbre Carve** – Mesh-Editor
  - **Timbre Stain** – Mal- und Texture-Editor (später: Malen direkt aufs Mesh über UVs)
- Anbindung an **Grove** (Web) und **Bridge** (Paddys Python-Server mit Realms).

## Architektur-Entscheidungen (fix)
- **Sprache:** Rust, Edition 2021, Cargo-Workspace unter `crates/`.
- **Rendering:** wgpu (nativ: DX12/Vulkan, Web: WebGPU). Fenster nativ: winit.
- **Scripting:** Jede Sprache ist ein **WASM-Gast**. Der Host kennt nur die Schnittstelle `wit/timbre.wit` (WebAssembly Component Model).
  - Nativ: wasmtime als Runtime.
  - Web: Browser-WASM, Komponenten über jco transpiliert.
  - Lua / Luau: Interpreter als WASM-Komponente, lädt Script-Quelltext zur Laufzeit (Hot Reload).
  - Python: componentize-py (groß → lazy laden).
  - Java: TeaVM → WASM; ggf. dünner Adapter nötig (Component-Model-Tooling für Java noch unreif).
  - Bonus-Sprachen: Rust, C, C++, C#, Go (TinyGo), Zig, AssemblyScript, JavaScript, Kotlin.
- **Zweite Steuer-Ebene:** Timbre wird ein **Bridge-Client** in einem Realm und bietet Funktionen an (Szene laden, Objekt spawnen, Mesh exportieren …). Asynchron, für Fernsteuerung aus jeder Sprache mit Bridge-Extension.
- **Assets:** eigenes Format `.tbr` (Mesh, Textur, Szene). Assets liegen lokal bzw. später auf Paddys Homeserver – **nicht** in Supabase. Supabase höchstens für Metadaten/Login (Grove-Auth).

## Aufbau des Repos
- `crates/timbre-tbr` – `.tbr` lesen/schreiben, **ohne Abhängigkeiten** (auch für Werkzeuge, Grove, Bridge). Spezifikation: `docs/tbr-format.md`.
- `crates/timbre-core` – Engine: Renderer, Zeit, Input, Kamera (später ECS, Scripting-Host).
- `crates/timbre-app` – Starter + Testszene (nativ und Browser via Trunk).
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
3. [~] Asset-Format `.tbr` v0 (Mesh, Textur, Szene) – wichtigste Entscheidung, alles baut darauf auf
4. [ ] ECS
5. [ ] WASM-Scripting-Host (wasmtime) + erste Sprache: **Luau**, dann **Lua**
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
- Meilenstein 3, Schritt 1 gebaut (wartet auf Validierung): `.tbr`-Format v0 mit Paddy abgestimmt – ein Container für alles (Kopf + Blöcke, unbekannte Blöcke werden übersprungen), Meshes als Vielecke, Texturen mit optionalen Ebenen, Szenen als Text, Verweise über 128-Bit-Asset-IDs, eigenes Paket `timbre-tbr`, Werkzeug `tbr`.
  - Testszene lädt Texturen jetzt aus `assets/texturen/*.tbr` (per `include_bytes!`, damit es auch im Browser ohne Dateisystem geht). `tbr mesh` an einem glTF mit zwei transformierten Würfeln getestet.
  - Offen für M3: Meshes werden noch nicht gezeichnet (braucht 3D-Renderer), Szenen-Inhalt kommt mit ECS, Laden zur Laufzeit statt `include_bytes!`, optionale Kompression (Flag im Kopf ist reserviert).
