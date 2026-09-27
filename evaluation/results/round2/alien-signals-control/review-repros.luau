local signals = require("../src")

local function show(name, actual, expected)
	print(name, "actual=" .. tostring(actual), "expected=" .. tostring(expected))
end

do
	local ok, err = pcall(function()
		signals.effect(function()
			error("initial sentinel")
		end)
	end)
	show("initial effect error", if ok then "no error" else tostring(err), "initial sentinel")
end

do
	local get, set = signals.signal(0)
	local runs = 0
	local stop = signals.effect(function()
		runs += 1
		get()
	end)
	signals.startBatch()
	set(1)
	stop()
	signals.endBatch()
	show("stopped queued effect runs", runs, 1)
end

do
	local get, set = signals.signal(0)
	local runs = 0
	signals.effect(function()
		signals.pauseTracking()
		signals.pauseTracking()
		signals.resumeTracking()
		signals.resumeTracking()
		get()
		runs += 1
	end)
	set(1)
	show("nested pause restoration runs", runs, 2)
end

do
	local get, set = signals.signal(0)
	local runs = 0
	local ok = pcall(function()
		signals.effectScope(function()
			signals.effect(function()
				get()
				runs += 1
			end)
			error("failed scope")
		end)
	end)
	set(1)
	show("failed scope retained effect", runs, 1)
end

do
	local get, set = signals.signal(0)
	local runs = 0
	local ok, err = pcall(function()
		signals.effect(function()
			get()
			runs += 1
			error("failed creation")
		end)
	end)
	pcall(function() set(1) end)
	show("failed effect retained subscription", runs, 1)
end

do
	local get, set = signals.signal(0)
	signals.effect(function()
		if get() == 1 then error("rerun sentinel") end
	end)
	local ok, err = pcall(function() set(1) end)
	show("rerun effect error", if ok then "no error" else tostring(err), "rerun sentinel")
end
