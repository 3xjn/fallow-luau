-- Dense branching for density / MI checks.
local function dense(a, b, c)
	if a and b or c then
		if a then
			return 1
		elseif b then
			return 2
		else
			return 3
		end
	end
	while a do
		a = false
	end
	return 0
end

return { dense = dense }
