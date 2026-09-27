-- Isolated review reproductions against unchanged production modules.
local realRequire = require
_G.Color3 = realRequire("@lune/roblox").Color3
getfenv().Color3 = _G.Color3

local proxyMeta = {}
proxyMeta.__index = function(self, key)
	if key == "Parent" then
		return setmetatable({ __path = self.__path:match("(.*)/[^/]+$") }, proxyMeta)
	end
	return setmetatable({ __path = self.__path .. "/" .. key }, proxyMeta)
end

local function shimRequire(target)
	if type(target) ~= "table" or not target.__path then
		return realRequire(target)
	end
	local previousScript = _G.script
	_G.script = setmetatable({ __path = target.__path }, proxyMeta)
	getfenv().script = _G.script
	local ok, result = pcall(realRequire, target.__path)
	_G.script = previousScript
	getfenv().script = previousScript
	if not ok then error(result, 0) end
	return result
end
_G.require = shimRequire
getfenv().require = shimRequire

local rules = shimRequire(setmetatable({ __path = "../src/shared/game/UIControlRules" }, proxyMeta))
local children = {}
for y = 1, 8 do
	for x = 1, 8 do
		table.insert(children, {
			className = "TextButton", isBoardCell = true, boardX = x, boardY = y,
		})
	end
end
local valid = rules.validateBoardGridChildren(children)
print("valid 8x8 grid:", valid)
children[1].boardX = 1.5
print("fractional coordinate accepted:", rules.validateBoardGridChildren(children))
children[1].boardX = 0 / 0
print("NaN coordinate accepted:", rules.validateBoardGridChildren(children))

local timestamps = {}
print("first request at uptime 0.1 accepted:", rules.consumeActionCooldown(timestamps, "play", 0.45, 0.1))
print("same request at uptime 0.6 accepted:", rules.consumeActionCooldown(timestamps, "play", 0.45, 0.6))

local persistence = shimRequire(setmetatable({ __path = "../src/shared/game/ProfilePersistence" }, proxyMeta))
local payload = { version = 2, coins = 10, fulfilledPurchaseIds = {}, redeemedCodes = {} }
local stored = { version = 3, coins = 20, futureField = "keep", fulfilledPurchaseIds = {}, redeemedCodes = {} }
print("future profile load status:", persistence.migrate(stored))
local reconciled = persistence.reconcileWithStored(payload, stored)
print("concurrent future profile rewritten as version:", reconciled.version, "future field:", reconciled.futureField)
