# Scripts in Timbre (Luau)

Scripts liegen im Projektordner unter `scripts/*.luau` und werden in **Timbre Studio**
geschrieben. **▶ Ausführen** (oder **F5**) startet alle Scripts, **■ Stopp** hält sie an und
setzt die Szene auf den Stand von vor dem Start zurück.

## Aufbau eines Scripts

```lua
-- läuft sofort beim Start (Variablen anlegen usw.)
local tempo = 200

function on_start()
	-- einmal nach dem Laden aller Scripts
end

function on_update(dt)
	-- 60-mal pro Sekunde; dt = Sekunden seit dem letzten Aufruf (1/60)
end
```

Jedes Script hat seine eigenen Variablen – zwei Scripts sehen die Variablen des anderen nicht.
Fehler stehen rot in der Konsole (mit Datei und Zeile). Ein Script mit Fehler in `on_update`
wird angehalten, die anderen laufen weiter. Endlosschleifen werden abgebrochen.

## Funktionen

Objekte sind Zahlen (IDs). `print(...)` schreibt in die Konsole.

| Funktion | Bedeutung |
|---|---|
| `timbre.log(...)` / `print(...)` | Text in die Konsole |
| `timbre.time()` | Sekunden seit ▶ |
| `timbre.spawn(name)` → id | Neues Objekt (unsichtbar, bis es ein Bild bekommt) |
| `timbre.find(name)` → id oder nil | Erstes Objekt mit diesem Namen |
| `timbre.destroy(id)` | Objekt löschen |
| `timbre.set_position(id, x, y)` | Position setzen |
| `timbre.get_position(id)` → x, y | Position lesen |
| `timbre.set_rotation(id, winkel)` | Drehung im Bogenmaß (`math.rad(90)` = 90°) |
| `timbre.get_rotation(id)` → winkel | Drehung lesen |
| `timbre.set_scale(id, x [, y])` | Skalierung |
| `timbre.set_velocity(id, vx, vy [, drehen])` | Bewegt sich ab jetzt von selbst (pro Sekunde) |
| `timbre.set_sprite(id, textur, breite [, höhe, ebene])` | Bild geben; Texturen: `"blatt"`, `"jahresringe"` |
| `timbre.set_color(id, r, g, b [, a])` | Einfärben, Werte 0–1 |
| `timbre.key_down(taste)` → bool | Taste gerade gedrückt |
| `timbre.key_pressed(taste)` → bool | Taste in diesem Schritt neu gedrückt |
| `timbre.mouse_position()` → x, y | Maus in Weltkoordinaten |
| `timbre.set_camera(x, y [, zoom])` | Kamera setzen |

Tastennamen: `"A"`–`"Z"`, `"0"`–`"9"`, `"Space"`, `"Enter"`, `"Escape"`, `"Shift"`, `"Ctrl"`,
`"Tab"`, `"Up"`, `"Down"`, `"Left"`, `"Right"` (Groß-/Kleinschreibung egal).

Koordinaten: Bei Zoom 1 ist eine Einheit ein Pixel, **y zeigt nach oben**.

## Luau

Luau ist die Lua-Variante von Roblox: `+=`, `-=`, `continue`, Typangaben (`local x: number`),
Strings mit `` `Wert: {x}` ``. Doku: https://luau.org
