-- Production module required by a test file for CRAP static_estimated coverage.
local function add(a, b)
	if a > b then
		return a
	elseif a < b then
		return b
	else
		return a + b
	end
end

return { add = add }
