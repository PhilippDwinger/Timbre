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

## Harte Regeln
- **Bridge niemals verändern.** Timbre spricht nur Bridges Protokoll als Client. Bridge-Anbindung erst, wenn Bridge 1.1 steht.
- Schnittstellenänderungen an `wit/timbre.wit` immer begründen und in diesem Dokument nachziehen – alle Sprachen hängen daran.
- Arbeitsweise: kleine Schritte, Paddy validiert jeden Schritt live, bevor es weitergeht. Neue Konzepte kurz erklären.
- Windows, Befehle für CMD (nicht PowerShell). Befehle immer mit echten Werten, keine Platzhalter.
- Nach jedem abgeschlossenen Meilenstein: Status unten aktualisieren.

## Meilensteine
1. [ ] Fenster + wgpu: farbiges Dreieck nativ, danach dasselbe im Browser (wasm32-unknown-unknown)
2. [ ] Fixed-Timestep Game Loop, Input, Kamera, Sprite-Rendering mit Batching
3. [ ] Asset-Format `.tbr` v0 (Mesh, Textur, Szene) – wichtigste Entscheidung, alles baut darauf auf
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
- Meilenstein 1a (nativ) gebaut, wartet auf Validierung: winit 0.30 + wgpu 30. `timbre-core/src/renderer.rs` kennt kein Fenster-Crate (bekommt nur ein `SurfaceTarget`), damit er später auch im Browser läuft. Dreieck kommt ohne Vertex-Buffer direkt aus `shaders/dreieck.wgsl`.
  - wgpu-30-Eigenheiten: `InstanceDescriptor::new_without_display_handle()`, `get_current_texture()` liefert `CurrentSurfaceTexture` (kein Result), Präsentieren über `queue.present(frame)`.
