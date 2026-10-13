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
| `timbre_laden` | `(i32 name, i32 name_n, i32 quelle, i32 quelle_n, i64 script_id, i64 eltern_id) → i32` | Script laden und Hauptteil ausführen; 0 ok, 1 Syntaxfehler, 2 Laufzeitfehler. IDs 0 = keins; der Gast stellt sie als `script.id` / `script.parent` bereit |
| `timbre_script_stoppen` | `(i64 script_id) → ()` | Script anhalten (sein Objekt wurde gelöscht) – seine wartenden Aufgaben (`task.wait`/`delay`) enden mit |
| `timbre_nachricht` | `(i32 name, i32 name_n, i32 art, f64 zahl, i32 text, i32 text_n) → ()` | Optional. `broadcast` aus einer **anderen** Sprache → `on_message` in allen Scripts (nicht weiterreichen). `art`: 0 nil, 1 bool (`zahl` 0/1), 2 Zahl, 3 Text, 4 Ganzzahl |
| `timbre_neu_laden` | wie `timbre_laden` | **Live-Neuladen** (optional): neue Fassung eines laufenden Scripts. Erst übersetzen – bei Syntaxfehler läuft die alte Fassung weiter (Rückgabe 1). Sonst alte Fassung(en) mit gleicher `script_id` anhalten, neue laden und, falls `timbre_start` schon lief, ihr `on_start` aufrufen |
| `timbre_beruehrung` | `(i64 script_id, i64 anderes) → ()` | Das Eltern-Objekt des Scripts hat `anderes` neu berührt → `on_touch` |

`timbre_update` weckt nach den `on_update`-Aufrufen auch fällige `task.wait`/`task.delay`-Aufgaben
(Zeit = Import `zeit`). `task`, `broadcast`/`on_message` und `shared` leben komplett im Gast.
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
| `destroy` | `(i64)` – löscht auch alle Kinder |
| `set_parent` | `(i64 id, i64 eltern)` – 0 = oberste Ebene; Schleifen werden ignoriert |
| `get_parent` | `(i64) → i64` – 0 = keins |
| `set_position` | `(i64, f32 x, f32 y)` |
| `get_position` | `(i64, i32 aus) → i32` – schreibt 2 × f32 nach `aus`; 0 = unbekanntes Objekt. Position/Drehung immer in Weltkoordinaten |
| `set_rotation` | `(i64, f32)` |
| `get_rotation` | `(i64) → f32` |
| `set_scale` | `(i64, f32 x, f32 y)` |
| `set_velocity` | `(i64, f32 vx, f32 vy, f32 drehen)` |
| `set_sprite` | `(i64, i32 p, i32 n, f32 b, f32 h, i32 ebene) → i32` – 0 = Textur unbekannt |
| `set_color` | `(i64, f32 r, f32 g, f32 b, f32 a)` |
| `key_down`, `key_pressed` | `(i32 p, i32 n) → i32` |
| `mouse_position` | `(i32 aus)` – 2 × f32 |
| `set_camera` | `(f32 x, f32 y, f32 zoom)` |
| `get_name` | `(i64, i32 aus, i32 platz) → i32` – Länge in Bytes (−1 = unbekannt); schreibt nur, wenn es passt |
| `set_collision` | `(i64, i32 form, i32 fest, i32 verankert)` – form 0 = entfernen, 1 = Kreis, 2 = Rechteck |
| `get_touching` | `(i64, i32 aus, i32 platz) → i32` – Anzahl; schreibt bis zu `platz` × i64 |
| `mouse_down`, `mouse_pressed` | `(i32 taste) → i32` – 0 links, 1 rechts, 2 Mitte |
| `ui_text` | `(i32 k, i32 kn, i32 t, i32 tn, f32 x, f32 y, f32 größe, f32 r, f32 g, f32 b, f32 a)` |
| `ui_clear` | `(i32 k, i32 kn)` – leerer Schlüssel = alle |
| `play_sound` | `(i32 p, i32 n, f32 lautstärke, i32 schleife) → i32` – 0 = Sound unbekannt; eine Endung im Namen wird ignoriert |
| `nachricht` | `(i32 p, i32 n, i32 art, f64 zahl, i32 t, i32 tn)` – jedes `broadcast` meldet sich hier zusätzlich, damit der Host es an die anderen Sprachen weiterreicht (Kodierung wie bei `timbre_nachricht`) |
| `stop_sounds` | `(i32 p, i32 n)` – leerer Name = alle |

Außerdem stellt der Host alle WASI-preview1-Funktionen bereit, die eine C-Laufzeit braucht – nur
lesend und nur für mitgegebene Dateien (siehe „Speicher-Dateisystem“ unten).

## Gäste

| Sprache | Modul | Quelle / Bauen |
|---|---|---|
| Luau | `assets/scripting/luau.wasm` | `tools/luau-gast/` (C++, Luau 0.22) |
| Lua 5.4 | `assets/scripting/lua.wasm` | `tools/lua-gast/` (dieselbe `gast.cpp`-Logik mit Lua-5.4-API, Lua 5.4.8 aus dem Crate `lua-src`) |
| Python 3.12 | `assets/scripting/python.wasm` + `python312.zip` | `tools/python-gast/` – `gast.c` (Modul `timbre`, Exporte) + `laufzeit.py` (Script-Verwaltung, async/await). CPython als fertige `libpython3.12.a` (WASI) von den *WebAssembly Language Runtimes* (VMware Labs), gelinkt mit wasi-sdk 20. Die Standardbibliothek liegt als Zip im **schreibgeschützten Speicher-Dateisystem** des Hosts (`/lib/python312.zip`) |

**Speicher-Dateisystem** (`timbre-core/src/wasi.rs`): Der Host bietet alle WASI-preview1-Funktionen an.
Lesen geht nur aus Dateien, die die Engine dem Gast mitgibt (`ScriptMotor::mit_dateien`), über ein
vorab geöffnetes `/` (fd 3). Alles, was schreibt, ändert oder ins Netz will, wird abgelehnt
(`EROFS`/`ENOTSUP`). stdout/stderr landen zeilenweise in der Konsole.

**Zwischenspeicher:** Übersetzte Gast-Module legt der Host im Temp-Ordner ab (`timbre-cache/<prüfsumme>.cwasm`),
damit Python beim zweiten Start sofort bereit ist.

Jede Sprache bekommt beim ▶ ihre eigene Laufzeit (eigene wasmtime-Instanz). Physik läuft einmal,
Berührungen werden an jede Laufzeit gemeldet. `broadcast` stellt jeder Gast selbst sofort zu (beliebige
Daten) und meldet es zusätzlich per Import `nachricht`; der Host reicht es nach dem Aufruf per
`timbre_nachricht` an die anderen Laufzeiten weiter (höchstens 8 Runden Hin und Her pro Schritt).
`shared` bleibt im jeweiligen Gast.

## Grenzen

- **Treibstoff:** 2 Mrd. Anweisungen für Laden/Start, 500 Mio. pro `update`. Wird es überschritten,
  bricht der Host ab und meldet „Endlosschleife?“. Danach ist der Gast unbrauchbar (■, ▶).
- Fehler im Script selbst fängt der Gast und meldet sie über `fehler`.

## Luau-Gast bauen

`tools/luau-gast/bauen.sh` (Linux/WSL, braucht `pip install ziglang`). Ergebnis:
`assets/scripting/luau.wasm` (liegt fertig im Repo). Besonderheiten: Luau nutzt C++-Ausnahmen
und longjmp → gebaut mit WebAssembly-Exceptions (neue exnref-Variante), die nötigen Teile von
libc++abi/libunwind werden mitübersetzt.
