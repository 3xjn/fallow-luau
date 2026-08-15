-- Cycle a → b → a
local b = require("./b")
return { a = b }
