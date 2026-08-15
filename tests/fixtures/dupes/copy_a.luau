-- Clone twin A
local function score(xs)
	local total = 0
	for i = 1, #xs do
		local v = xs[i]
		if v ~= nil then
			total = total + v
		end
	end
	if total < 0 then
		return -total
	else
		return total
	end
end

return { score = score }
