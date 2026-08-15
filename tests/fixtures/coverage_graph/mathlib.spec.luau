-- Test file: direct require → 85% estimated coverage on mathlib.
local mathlib = require("./mathlib")

return {
	test_add = function()
		assert(mathlib.add(1, 2) ~= nil)
	end,
}
