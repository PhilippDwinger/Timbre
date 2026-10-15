-- Eisfeld: 3D-Beispiel und Live-Script.
--
-- Oben in der Leiste auf "3D" schalten. Mit "▶ Run" bei eisberge(...) entstehen Eisberge
-- rund um die Mitte – ohne das Spiel zu starten. Beim Ausführen (▶) dreht sich der Kristall.

live function eisberge(anzahl, radius)
	anzahl = anzahl or 12
	radius = radius or 8
	local ordner = timbre.find("Eisberge") or timbre.spawn("Eisberge")
	for i = 1, anzahl do
		local winkel = i / anzahl * math.pi * 2
		local hoehe = 0.6 + math.random() * 2.2
		local berg = timbre.spawn("Eisberg " .. i)
		timbre.set_parent(berg, ordner)
		local blau = 0.85 + math.random() * 0.15
		timbre.set_mesh(berg, if i % 3 == 0 then "sphere" else "cube", 0.62, 0.82, blau)
		local r = radius + (math.random() - 0.5) * 2
		timbre.set_position3d(berg, math.cos(winkel) * r, hoehe / 2, math.sin(winkel) * r)
		timbre.set_scale3d(berg, 0.8 + math.random(), hoehe, 0.8 + math.random())
		timbre.set_rotation3d(berg, 0, math.random() * 90, 0)
	end
	print(`{anzahl} Eisberge`)
end

live function schmelzen()
	local ordner = timbre.find("Eisberge")
	if ordner then
		timbre.destroy(ordner)
		print("Alles geschmolzen")
	end
end

local kristall = timbre.find("Kristall")

function on_update(dt)
	if kristall then
		local _, y, _ = timbre.get_rotation3d(kristall)
		timbre.set_rotation3d(kristall, 35, y + 60 * dt, 45)
		local x, _, z = timbre.get_position3d(kristall)
		timbre.set_position3d(kristall, x, 2.2 + math.sin(timbre.time() * 2) * 0.3, z)
	end
end
