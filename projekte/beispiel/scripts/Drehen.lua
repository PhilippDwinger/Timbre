-- Drehen: ein Script für die Property "Scripts".
--
-- Es hängt an keinem Objekt, sondern wird von Objekten verknüpft (Eigenschaften →
-- Property hinzufügen → Scripts). Für jedes verknüpfte Objekt läuft eine eigene Kopie,
-- und script.object ist jeweils dieses Objekt. Hier: Baumstumpf 1, 3 und 5.

local tempo = 0.4

function on_update(dt)
	local w = timbre.get_rotation(script.object)
	timbre.set_rotation(script.object, w + tempo * dt)
end
