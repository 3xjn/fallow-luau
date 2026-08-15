-- Reachable module with one used and one unused returned key.
return {
	keep = function()
		return 1
	end,
	drop_me = function()
		return 0
	end,
}
