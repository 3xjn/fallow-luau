local m = require("./module")
m:keep()
local _constant = m["constant"]
return require("./module").keep()
