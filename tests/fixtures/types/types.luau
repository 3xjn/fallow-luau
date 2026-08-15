-- Unused type declaration.
type NeverUsed = { x: number }
type Used = number
local function f(a: Used): Used
	return a
end

return { f = f }
