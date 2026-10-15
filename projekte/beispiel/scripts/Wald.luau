-- Wald: ein Live-Script.
--
-- "live" vor function macht aus den Argumenten Felder in den Eigenschaften dieses Scripts.
-- Mit "▶ Run" wird die Funktion sofort aufgerufen – ohne das Spiel zu starten.
-- So kann man Level-Teile erzeugen und ausprobieren. Strg+Z macht es wieder rückgängig.

live function pflanze_ring(anzahl, radius, groesse)
	anzahl = anzahl or 8
	radius = radius or 600
	groesse = groesse or 90
	local ordner = timbre.find("Waldring") or timbre.spawn("Waldring")
	for i = 1, anzahl do
		local winkel = i / anzahl * math.pi * 2
		local baum = timbre.spawn("Baum " .. i)
		timbre.set_parent(baum, ordner)
		timbre.set_sprite(baum, "jahresringe", groesse, groesse, -1)
		timbre.set_position(baum, math.cos(winkel) * radius, math.sin(winkel) * radius)
		timbre.set_collision(baum, "circle", true, true)
	end
	print(`{anzahl} Bäume gepflanzt`)
end

live function faellen()
	local ordner = timbre.find("Waldring")
	if ordner then
		timbre.destroy(ordner)
		print("Wald gefällt")
	end
end
