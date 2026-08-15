-- Large outer file with a ~40-line nested local function that must still be scored.
local function pad()
	-- filler so the file is large while the interesting unit is nested
	local t = {}
	for i = 1, 80 do
		t[i] = i
	end
	return t
end

local function outer(flag, values)
	local function inner(xs)
		local total = 0
		if not xs then
			return 0
		end
		for i = 1, #xs do
			local v = xs[i]
			if v == nil then
				total = total + 0
			elseif v < 0 then
				if flag then
					total = total - v
				else
					total = total + v
				end
			elseif v == 0 then
				total = total
			elseif v > 10 then
				if flag and v > 20 then
					total = total + v * 2
				elseif flag then
					total = total + v
				else
					total = total + 1
				end
			else
				total = total + v
			end
			if total > 1000 then
				break
			end
		end
		if total < 0 then
			return -total
		elseif total == 0 then
			return 0
		else
			return total
		end
	end

	local _ = pad()
	return inner(values)
end

-- More filler lines to keep the file large relative to the nested unit.
local filler = {
	1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
	11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
	21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
	31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
	41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
}

	local _pad_0 = 0
	local _pad_1 = 1
	local _pad_2 = 2
	local _pad_3 = 3
	local _pad_4 = 4
	local _pad_5 = 5
	local _pad_6 = 6
	local _pad_7 = 7
	local _pad_8 = 8
	local _pad_9 = 9
	local _pad_10 = 10
	local _pad_11 = 11
	local _pad_12 = 12
	local _pad_13 = 13
	local _pad_14 = 14
	local _pad_15 = 15
	local _pad_16 = 16
	local _pad_17 = 17
	local _pad_18 = 18
	local _pad_19 = 19
	local _pad_20 = 20
	local _pad_21 = 21
	local _pad_22 = 22
	local _pad_23 = 23
	local _pad_24 = 24
	local _pad_25 = 25
	local _pad_26 = 26
	local _pad_27 = 27
	local _pad_28 = 28
	local _pad_29 = 29
	local _pad_30 = 30
	local _pad_31 = 31
	local _pad_32 = 32
	local _pad_33 = 33
	local _pad_34 = 34
	local _pad_35 = 35
	local _pad_36 = 36
	local _pad_37 = 37
	local _pad_38 = 38
	local _pad_39 = 39
	local _pad_40 = 40
	local _pad_41 = 41
	local _pad_42 = 42
	local _pad_43 = 43
	local _pad_44 = 44
	local _pad_45 = 45
	local _pad_46 = 46
	local _pad_47 = 47
	local _pad_48 = 48
	local _pad_49 = 49

return {
	outer = outer,
	filler = filler,
}
