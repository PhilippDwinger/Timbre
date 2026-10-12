-- Spieler: mit WASD oder den Pfeiltasten steuern. Die Kamera folgt.
--
-- Jedes Script kann zwei Funktionen haben:
--   on_start()      läuft einmal, wenn du auf ▶ drückst
--   on_update(dt)   läuft 60-mal pro Sekunde; dt = Sekunden seit dem letzten Mal

local TEMPO = 320 -- Welteinheiten pro Sekunde
local spieler

function on_start()
	spieler = timbre.find("Spieler")
	if not spieler then
		print("Kein Objekt 'Spieler' in der Szene – lege eins an!")
	end
end

function on_update(dt)
	if not spieler then
		return
	end

	local dx, dy = 0, 0
	if timbre.key_down("W") or timbre.key_down("Up") then dy += 1 end
	if timbre.key_down("S") or timbre.key_down("Down") then dy -= 1 end
	if timbre.key_down("A") or timbre.key_down("Left") then dx -= 1 end
	if timbre.key_down("D") or timbre.key_down("Right") then dx += 1 end

	local x, y = timbre.get_position(spieler)
	if dx ~= 0 or dy ~= 0 then
		local laenge = math.sqrt(dx * dx + dy * dy)
		x += dx / laenge * TEMPO * dt
		y += dy / laenge * TEMPO * dt
		timbre.set_position(spieler, x, y)
		-- Das Blatt zeigt in Laufrichtung (Textur zeigt nach oben → 90° abziehen)
		timbre.set_rotation(spieler, math.atan2(dy, dx) - math.pi / 2)
	end

	timbre.set_camera(x, y, 1)
end
