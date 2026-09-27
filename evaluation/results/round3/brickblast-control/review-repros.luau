local roblox = require("@lune/roblox")
for _, name in { "Vector2", "Color3" } do
	_G[name] = roblox[name]
	getfenv()[name] = roblox[name]
end

local realRequire = require
local proxyMeta = {}
proxyMeta.__index = function(self, key)
	if key == "Parent" then
		return setmetatable({ __path = self.__path:match("(.*)/[^/]+$") or "." }, proxyMeta)
	end
	return setmetatable({ __path = self.__path .. "/" .. key }, proxyMeta)
end
local function proxy(path)
	return setmetatable({ __path = path }, proxyMeta)
end
local function shimRequire(target)
	if type(target) ~= "table" or not target.__path then
		return realRequire(target)
	end
	local previous = _G.script
	_G.script = proxy(target.__path)
	getfenv().script = _G.script
	local ok, result = pcall(realRequire, target.__path)
	_G.script = previous
	getfenv().script = previous
	if not ok then error(result, 0) end
	return result
end
_G.require = shimRequire
getfenv().require = shimRequire

local game = "../src/shared/game"
local Grid = shimRequire(proxy(game .. "/Grid"))
local Config = shimRequire(proxy(game .. "/Config"))
local UIControlRules = shimRequire(proxy(game .. "/UIControlRules"))
local Blocks = shimRequire(proxy(game .. "/Blocks"))
local DeveloperTools = shimRequire(proxy(game .. "/DeveloperTools"))
local StageProgression = shimRequire(proxy(game .. "/StageProgression"))
local board = Grid.create()
local single = { id = "single", color = "red", cells = { Vector2.new(0, 0) } }
local legal = Grid.canPlace(board, single, 1.5, 1)
local placed, count = Grid.place(board, single, 1.5, 1)
print("fractional placement:", legal, placed, count, board[1][1.5], board[1][1], board[1][2])

local children = {}
for y = 1, Config.BoardSize do
	for x = 1, Config.BoardSize do
		table.insert(children, { className = "TextButton", isBoardCell = true, boardX = x, boardY = y })
	end
end
children[1].boardX = 1.5
local ok, problems, count = UIControlRules.validateBoardGridChildren(children)
print("fractional board child:", ok, #problems, count)
children[1].boardX = 0 / 0
ok, problems, count = UIControlRules.validateBoardGridChildren(children)
print("NaN board child:", ok, #problems, count)

local setup = DeveloperTools.boardPattern("perfectClearSetup")
local winning = {}
for _, definition in Blocks.Shapes do
	for y = 1, Config.BoardSize do
		for x = 1, Config.BoardSize do
			local nextBoard = Grid.clone(setup)
			local canPlace = Grid.place(nextBoard, definition, x, y)
			if canPlace then
				local lines = Grid.clearLines(nextBoard)
				if lines > 0 and StageProgression.isBoardEmpty(nextBoard) then
					table.insert(winning, definition.id .. "@" .. x .. "," .. y)
				end
			end
		end
	end
end
print("perfectClearSetup one-move perfect clears:", #winning, table.concat(winning, ";"))
