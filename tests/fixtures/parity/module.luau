local M = {}
function M:keep() return 1 end
function M.drop() return 2 end
return { keep = M.keep, drop = M.drop, constant = 3 }
