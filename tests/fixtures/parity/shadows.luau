local m = require("./module")
do
    local m = {drop = function() return 0 end}
    m.drop()
end
local function shadow(m)
    return m.drop()
end
return shadow
