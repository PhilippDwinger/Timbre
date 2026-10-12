-- Beispiel: so soll ein Timbre-Script in Luau aussehen (läuft ab Meilenstein 5)
local kiste

function on_start()
    kiste = timbre.spawn("Kiste")
    timbre.log("Kiste erzeugt: " .. kiste)
end

function on_update(dt)
    local t = timbre.time()
    timbre.set_position(kiste, math.sin(t) * 2, 0, 0)
end
