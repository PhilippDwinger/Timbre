-- Zeit: ein Script in Lua 5.4 (die anderen Scripts sind Luau).
-- Zeigt die Spielzeit unter den Goldstücken an – und wann zuletzt eins gesammelt wurde.
--
-- Lua und Luau sind fast gleich. Was es in Lua NICHT gibt:
--   x += 1        → schreibe x = x + 1
--   continue      → mit goto oder if lösen
--   `Text {x}`    → string.format("Text %d", x) oder "Text " .. x
-- Dafür hat Lua 5.4 echte Ganzzahlen (math.type(3) == "integer") und // für ganzzahlige Division.

local sekunden = 0
local letztes = nil

local function uhrzeit(s)
	local ganz = math.floor(s)
	return string.format("%d:%02d", ganz // 60, ganz % 60)
end

function on_update(dt)
	sekunden = sekunden + dt
	local text = "Zeit: " .. uhrzeit(sekunden)
	if letztes then
		text = text .. "   ·   letztes Goldstück bei " .. uhrzeit(letztes)
	end
	timbre.ui_text("zeit", text, 16, 84, 18, 0.75, 0.88, 1)
end

-- Nachrichten kommen auch aus Scripts anderer Sprachen: "gesammelt" schickt Steuerung.luau.
function on_message(nachricht)
	if nachricht == "gesammelt" then
		letztes = sekunden
	end
end
