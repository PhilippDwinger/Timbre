-- Beispiel: eine Baumscheibe, die hin und her schwingt und sich dreht
local kiste

function on_start()
	kiste = timbre.spawn("Kiste")
	timbre.set_sprite(kiste, "jahresringe", 80)
	timbre.log("Kiste erzeugt: " .. kiste)
end

function on_update(dt)
	local t = timbre.time()
	timbre.set_position(kiste, math.sin(t) * 200, 0)
	timbre.set_rotation(kiste, t)
end
