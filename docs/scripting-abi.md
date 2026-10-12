# Scripting-Schnittstelle (ABI) – für Sprach-Gäste

Jede Script-Sprache ist ein **WebAssembly-Modul** („Gast“), das die Engine („Host“) lädt.
Nativ führt **wasmtime** die Module aus, im Browser später die eingebaute WebAssembly-Laufzeit.

Entscheidung (Meilenstein 5): **einfache Core-Module mit schmaler C-Schnittstelle** statt
Component Model. Grund: Ein Host-Code für nativ und Browser, keine Adapter, sehr kleine
Schnittstelle. `wit/timbre.wit` beschreibt dieselben Funktionen als Dokumentation.

## Exporte des Gasts

| Funktion | Signatur | Bedeutung |
|---|---|---|
| `memory` | Speicher | gemeinsamer Speicher für Texte |
| `_initialize` | `() → ()` | optional, einmal nach dem Laden (C-Laufzeit) |
| `timbre_alloc` | `(i32 n) → i32 ptr` | Speicher für Texte vom Host |
| `timbre_free` | `(i32 ptr) → ()` | wieder freigeben |
| `timbre_laden` | `(i32 name, i32 name_n, i32 quelle, i32 quelle_n) → i32` | Script laden und Hauptteil ausführen; 0 ok, 1 Syntaxfehler, 2 Laufzeitfehler |
| `timbre_start` | `() → ()` | `on_start` aller Scripts |
| `timbre_update` | `(f32 dt) → ()` | `on_update(dt)` aller Scripts |

Texte sind UTF-8, übergeben als (Zeiger, Länge) im Gast-Speicher.

## Importe (Modul `timbre`)

Objekt-IDs sind `i64` (intern: `hecs::Entity::to_bits`). Unbekannte IDs werden ignoriert.

| Funktion | Signatur |
|---|---|
| `log`, `fehler` | `(i32 p, i32 n)` |
| `zeit` | `() → f64` |
| `spawn`, `find` | `(i32 p, i32 n) → i64` (find: 0 = nicht gefunden) |
| `destroy` | `(i64)` |
| `set_position` | `(i64, f32 x, f32 y)` |
| `get_position` | `(i64, i32 aus) → i32` – schreibt 2 × f32 nach `aus`; 0 = unbekanntes Objekt |
| `set_rotation` | `(i64, f32)` |
| `get_rotation` | `(i64) → f32` |
| `set_scale` | `(i64, f32 x, f32 y)` |
| `set_velocity` | `(i64, f32 vx, f32 vy, f32 drehen)` |
| `set_sprite` | `(i64, i32 p, i32 n, f32 b, f32 h, i32 ebene) → i32` – 0 = Textur unbekannt |
| `set_color` | `(i64, f32 r, f32 g, f32 b, f32 a)` |
| `key_down`, `key_pressed` | `(i32 p, i32 n) → i32` |
| `mouse_position` | `(i32 aus)` – 2 × f32 |
| `set_camera` | `(f32 x, f32 y, f32 zoom)` |

Außerdem stellt der Host die wenigen WASI-Funktionen bereit, die eine C-Laufzeit braucht
(`fd_write`, `fd_close`, `fd_seek`, `fd_fdstat_get`, `clock_time_get`). Dateizugriffe werden abgelehnt.

## Grenzen

- **Treibstoff:** 2 Mrd. Anweisungen für Laden/Start, 500 Mio. pro `update`. Wird es überschritten,
  bricht der Host ab und meldet „Endlosschleife?“. Danach ist der Gast unbrauchbar (■, ▶).
- Fehler im Script selbst fängt der Gast und meldet sie über `fehler`.

## Luau-Gast bauen

`tools/luau-gast/bauen.sh` (Linux/WSL, braucht `pip install ziglang`). Ergebnis:
`assets/scripting/luau.wasm` (liegt fertig im Repo). Besonderheiten: Luau nutzt C++-Ausnahmen
und longjmp → gebaut mit WebAssembly-Exceptions (neue exnref-Variante), die nötigen Teile von
libc++abi/libunwind werden mitübersetzt.
