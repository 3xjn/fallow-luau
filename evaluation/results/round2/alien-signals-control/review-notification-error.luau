local signals = require("../src")

local get, set = signals.signal(0)
local firstRuns = 0
local secondRuns = 0

signals.effect(function()
	firstRuns += 1
	if get() == 1 then
		error("notification sentinel")
	end
end)

signals.effect(function()
	get()
	secondRuns += 1
end)

local ok, err = pcall(function()
	set(1)
end)
print("set error", tostring(err))
print("runs after set", firstRuns, secondRuns, "expected", 2, 2)

local unrelated, setUnrelated = signals.signal(0)
local unrelatedRuns = 0
signals.effect(function()
	unrelated()
	unrelatedRuns += 1
end)
setUnrelated(1)
print("runs after unrelated set", firstRuns, secondRuns, unrelatedRuns, "expected", 2, 2, 2)

local ok2, err2 = pcall(function()
	set(2)
end)
print("next set error", tostring(err2))
print("runs after next set", firstRuns, secondRuns, "expected", 3, 3)
