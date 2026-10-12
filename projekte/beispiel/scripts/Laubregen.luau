-- Laubregen: lässt rund um den Spieler Blätter fallen.
-- Leertaste = Windstoß.

local FARBEN = {
	{ 0.87, 0.62, 0.38 },
	{ 0.80, 0.70, 0.20 },
	{ 0.85, 0.40, 0.20 },
	{ 0.45, 0.65, 0.30 },
}
local MAX_BLAETTER = 400

local blaetter = {}
local warten = 0

-- Ein Ordner im Explorer für alle Blätter (Objekt ohne Position = unsichtbar)
local ordner = timbre.spawn("Laub")
timbre.set_parent(ordner, script.id)

local function neues_blatt(x, y)
	local blatt = timbre.spawn("Laub")
	local groesse = math.random(14, 30)
	timbre.set_sprite(blatt, "blatt", groesse, groesse, 1)
	local f = FARBEN[math.random(#FARBEN)]
	timbre.set_color(blatt, f[1], f[2], f[3])
	timbre.set_position(blatt, x, y)
	timbre.set_rotation(blatt, math.random() * math.pi * 2)
	timbre.set_velocity(blatt, math.random(-40, 40), -math.random(60, 140), (math.random() - 0.5) * 4)
	timbre.set_parent(blatt, ordner)
	table.insert(blaetter, blatt)
end

function on_update(dt)
	local spieler = timbre.find("Spieler")
	local mx, my = 0, 0
	if spieler then
		mx, my = timbre.get_position(spieler)
	end

	-- Alle 0,05 Sekunden ein neues Blatt oberhalb des Bildausschnitts
	warten -= dt
	if warten <= 0 and #blaetter < MAX_BLAETTER then
		neues_blatt(mx + math.random(-700, 700), my + 450)
		warten = 0.05
	end

	if timbre.key_pressed("Space") then
		for _, blatt in blaetter do
			timbre.set_velocity(blatt, math.random(250, 450), math.random(-60, 120), math.random(-6, 6))
		end
		print("Windstoß!")
	end

	-- Blätter, die weit unten sind, wieder entfernen (rückwärts, damit das Löschen klappt)
	for i = #blaetter, 1, -1 do
		local _, y = timbre.get_position(blaetter[i])
		if y < my - 600 then
			timbre.destroy(blaetter[i])
			table.remove(blaetter, i)
		end
	end
end
