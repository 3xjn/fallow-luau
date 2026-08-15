-- Dynamic require and loadstring are unresolved edges, not reachability.
local name = "mod"
local m = require(name)
local chunk = loadstring("return 1")
return { m = m, chunk = chunk }
