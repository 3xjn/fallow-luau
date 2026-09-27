local signals = require("../src")

local function case(name, fn)
	local ok, result = pcall(fn)
	print(name, ok, result)
end

case("initial effect error", function()
	local ok, err = pcall(function()
		signals.effect(function()
			error("effect failure")
		end)
	end)
	return tostring(ok) .. " | " .. tostring(err)
end)

case("computed error", function()
	local value = signals.computed(function()
		error("computed failure")
	end)
	local ok, err = pcall(value)
	return tostring(ok) .. " | " .. tostring(err)
end)

case("rerun effect error", function()
	local getValue, setValue = signals.signal(0)
	signals.effect(function()
		if getValue() == 1 then
			error("rerun failure")
		end
	end)
	local ok, err = pcall(function()
		setValue(1)
	end)
	return tostring(ok) .. " | " .. tostring(err)
end)

case("scope error", function()
	local ok, err = pcall(function()
		signals.effectScope(function()
			error("scope failure")
		end)
	end)
	return tostring(ok) .. " | " .. tostring(err)
end)

case("nested pause", function()
	local getValue, setValue = signals.signal(0)
	local runs = 0
	signals.effect(function()
		runs += 1
		signals.pauseTracking()
		signals.pauseTracking()
		signals.resumeTracking()
		getValue()
		signals.resumeTracking()
	end)
	setValue(1)
	return runs
end)

case("nested scope disposal", function()
	local getValue, setValue = signals.signal(0)
	local runs = 0
	local stopOuter = signals.effectScope(function()
		signals.effectScope(function()
			signals.effect(function()
				getValue()
				runs += 1
			end)
		end)
	end)
	stopOuter()
	setValue(1)
	return runs
end)

case("direct scope disposal control", function()
	local getValue, setValue = signals.signal(0)
	local runs = 0
	local stop = signals.effectScope(function()
		signals.effect(function()
			getValue()
			runs += 1
		end)
	end)
	stop()
	setValue(1)
	return runs
end)
