-- Feature flag patterns for flags fixture.
local FF_NEW_UI = true
local Settings = { CombatEnabled = true }

local function tick()
	if FF_NEW_UI then
		return "new"
	end
	if Settings.CombatEnabled then
		return "combat"
	end
	if GetAttribute("BetaMode") then
		return "beta"
	end
	return "old"
end

return { tick = tick }
