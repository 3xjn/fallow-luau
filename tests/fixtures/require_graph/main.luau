-- Tiny string-literal require graph.
local util = require("./util")
return {
	double = function(x)
		return util.add(x, x)
	end,
}
