-- Sammelspiel: Goldstücke einsammeln!
--
-- Zeigt, wie Scripts zusammenarbeiten:
--   * dieses Script verteilt Goldstücke und zählt die Punkte
--   * "Steuerung" (unter dem Spieler) merkt per on_touch, wenn der Spieler ein Goldstück
--     berührt, und meldet es mit timbre.broadcast("gesammelt")
--   * task.wait / task.delay für Zeitabläufe, timbre.ui_text für die Anzeige

local ANZAHL = 8
local BEREICH = 700
local punkte = 0
local ordner

local function goldstueck()
	local g = timbre.spawn("Goldstück")
	timbre.set_parent(g, ordner)
	timbre.set_sprite(g, "jahresringe", 30, 30, 1)
	timbre.set_color(g, 1.0, 0.82, 0.25)
	timbre.set_position(g, math.random(-BEREICH, BEREICH), math.random(-BEREICH, BEREICH) * 0.6)
	timbre.set_velocity(g, 0, 0, 2) -- dreht sich langsam
	timbre.set_collision(g, "circle", false) -- nur Auslöser: blockiert nichts, meldet aber Berührungen
end

local function anzeigen()
	timbre.ui_text("punkte", `Goldstücke: {punkte}`, 16, 12, 26, 1, 0.85, 0.3)
end

function on_start()
	ordner = timbre.spawn("Gold")
	for i = 1, ANZAHL do
		goldstueck()
	end
	anzeigen()
	timbre.ui_text("hilfe", "WASD laufen · Leertaste Windstoß", 16, 48, 15, 1, 1, 1, 0.7)
	task.wait(4)
	timbre.ui_clear("hilfe")
end

function on_message(nachricht)
	if nachricht ~= "gesammelt" then
		return
	end
	punkte += 1
	anzeigen()
	if punkte % 5 == 0 then
		timbre.ui_text("lob", "Super!", 16, 48, 24, 0.55, 1, 0.5)
		task.delay(1.5, function()
			timbre.ui_clear("lob")
		end)
	end
	task.wait(1)
	goldstueck() -- eine Sekunde später taucht ein neues auf
end
