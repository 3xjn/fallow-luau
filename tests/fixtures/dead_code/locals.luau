-- Unused local binding.
local function entry()
	local never_read = 42
	return 1
end

return { entry = entry }
