# Scripts in Timbre (Luau)

Scripts sind **Objekte** in der Szene – wie in Roblox. Sie stehen im **Explorer**, gern unter
dem Objekt, das sie steuern sollen. Der Quelltext liegt als Datei unter `scripts/<Name>.luau`.

- **Neues Script:** „+ Script“ im Explorer (landet unter dem ausgewählten Objekt) oder
  Rechtsklick auf ein Objekt → „Script hinzufügen“.
- **Öffnen:** Doppelklick im Explorer.
- **Eigenschaften:** Name (benennt auch die Datei um), Eltern, Aktiviert.
- **Löschen:** Entf oder Rechtsklick → Löschen. Beim Speichern wandert die Datei nach
  `scripts/.papierkorb/` – nichts geht verloren.

**▶ Ausführen** (oder **F5**) startet alle aktivierten Scripts, **■ Stopp** hält sie an und
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

Weitere Funktionen, die Timbre aufruft, wenn es sie gibt:

| Funktion | Wann |
|---|---|
| `on_touch(anderes)` | Das Objekt, unter dem das Script hängt (`script.parent`), hat ein anderes Objekt berührt. Beide brauchen eine **Kollision**. |
| `on_message(nachricht, daten)` | Ein Script hat `timbre.broadcast(nachricht, daten)` gerufen. |

Jedes Script hat seine eigenen Variablen – zwei Scripts sehen die Variablen des anderen nicht.
Für Gemeinsames gibt es die Tabelle **`shared`** (z. B. `shared.punkte = 0`), die alle Scripts sehen.

## Warten und Zeitabläufe (`task`)

Wie in Roblox:

| Funktion | Bedeutung |
|---|---|
| `task.wait(sekunden)` | Pause – geht in `on_start`, `on_touch`, `on_message`, `task.spawn` und `task.delay`, **nicht** in `on_update` |
| `task.delay(sekunden, funktion)` | Funktion später einmal ausführen |
| `task.spawn(funktion, ...)` | Funktion sofort als eigene Aufgabe starten (darf `task.wait` benutzen) |

```lua
function on_start()
	while true do
		task.wait(2)
		print("alle 2 Sekunden")
	end
end
```

Jedes Script kennt sich selbst über `script`:

| Feld | Bedeutung |
|---|---|
| `script.name` | Name des Script-Objekts |
| `script.id` | Objekt-ID des Scripts |
| `script.parent` | Objekt, unter dem das Script hängt (oder `nil`) – wie `script.Parent` in Roblox |

Wird das Script-Objekt gelöscht (z. B. zusammen mit seinem Eltern-Objekt), hört das Script auf.
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
| `timbre.destroy(id)` | Objekt löschen (samt Kindern) |
| `timbre.set_parent(id, eltern)` | Unter ein anderes Objekt hängen (`nil` = oberste Ebene) |
| `timbre.get_parent(id)` → id oder nil | Eltern-Objekt |
| `timbre.set_position(id, x, y)` | Position setzen |
| `timbre.get_position(id)` → x, y | Position lesen |
| `timbre.set_rotation(id, winkel)` | Drehung im Bogenmaß (`math.rad(90)` = 90°) |
| `timbre.get_rotation(id)` → winkel | Drehung lesen |
| `timbre.set_scale(id, x [, y])` | Skalierung |
| `timbre.set_velocity(id, vx, vy [, drehen])` | Bewegt sich ab jetzt von selbst (pro Sekunde) |
| `timbre.set_sprite(id, textur, breite [, höhe, ebene])` | Bild geben; Texturen: `"blatt"`, `"jahresringe"` |
| `timbre.set_color(id, r, g, b [, a])` | Einfärben, Werte 0–1 |
| `timbre.get_name(id)` → text oder nil | Name eines Objekts |
| `timbre.set_collision(id, form [, fest, verankert])` | Kollision: Form `"circle"` oder `"box"` (`nil` = entfernen); `fest=false` = nur Auslöser (meldet, blockiert nicht); `verankert=false` = wird weggeschoben. Größe = Bildgröße |
| `timbre.get_touching(id)` → Tabelle | Alle Objekte, die das Objekt gerade berührt |
| `timbre.broadcast(nachricht [, daten])` | Ruft `on_message(nachricht, daten)` in allen Scripts auf |
| `timbre.ui_text(schlüssel, text, x, y [, größe, r, g, b, a])` | Text über dem Spiel (x/y in Pixeln ab links oben); gleicher Schlüssel = Text ersetzen |
| `timbre.ui_clear([schlüssel])` | Text entfernen (ohne Schlüssel: alle) |
| `timbre.mouse_down(taste)` / `timbre.mouse_pressed(taste)` → bool | Maustaste `"left"`, `"right"`, `"middle"` gedrückt / neu gedrückt (nur wenn die Maus über dem Spiel ist) |
| `timbre.key_down(taste)` → bool | Taste gerade gedrückt |
| `timbre.key_pressed(taste)` → bool | Taste in diesem Schritt neu gedrückt |
| `timbre.mouse_position()` → x, y | Maus in Weltkoordinaten |
| `timbre.set_camera(x, y [, zoom])` | Kamera setzen |

Tastennamen: `"A"`–`"Z"`, `"0"`–`"9"`, `"Space"`, `"Enter"`, `"Escape"`, `"Shift"`, `"Ctrl"`,
`"Tab"`, `"Up"`, `"Down"`, `"Left"`, `"Right"` (Groß-/Kleinschreibung egal).

Koordinaten: Bei Zoom 1 ist eine Einheit ein Pixel, **y zeigt nach oben**.

**Kinder bewegen sich mit ihrem Eltern-Objekt.** `get_position`/`set_position` und
`get_rotation`/`set_rotation` arbeiten trotzdem immer mit der Lage **in der Welt** – du musst
nicht umrechnen. (Im Eigenschaften-Fenster des Studios stehen dagegen die Werte relativ zum
Eltern-Objekt.) `set_parent` hängt um, ohne dass das Objekt springt.

## Kollisionen

Objekte mit Bild bekommen im Studio unter **Eigenschaften → + Kollision** (oder per
`timbre.set_collision`) eine Form. **Fest** blockiert, sonst ist es ein **Auslöser** (z. B. Münzen).
**Verankert** = wird nie weggeschoben (Wände). Rechtecke werden bei Kollisionen nicht gedreht.
Feste, nicht verankerte Objekte (z. B. der Spieler) werden aus festen Objekten herausgeschoben.

Beispiel: das Sammelspiel im Beispielprojekt (`Sammelspiel` + `Steuerung` unter `Spieler`).

## Luau

Luau ist die Lua-Variante von Roblox: `+=`, `-=`, `continue`, Typangaben (`local x: number`),
Strings mit `` `Wert: {x}` ``. Doku: https://luau.org
