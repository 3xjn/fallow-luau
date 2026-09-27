-- Executable review evidence against unchanged production modules.
local FlagCache = require("../src/Server/FlagCache")
local EventQueue = require("../src/Server/EventQueue")
local RetryPolicy = require("../src/Shared/RetryPolicy")

local store = FlagCache.empty()
FlagCache.applyResponse(store, {
    featureFlags = { retired = true, live = true },
    featureFlagPayloads = { retired = { old = true } },
})
FlagCache.applyResponse(store, { featureFlags = { live = false } })
print("flag reload retired value:", tostring(FlagCache.getValue(store, "retired")))
print("flag reload retired payload:", tostring(FlagCache.getPayload(store, "retired") ~= nil))
print("flag reload live value:", tostring(FlagCache.getValue(store, "live")))

local sent = {}
local queue
local transport = {
    sendBatch = function(_, payload)
        for _, event in payload.batch do
            table.insert(sent, event.event)
        end
        if #sent == 2 then
            -- Models a new capture during a yielding RequestAsync at queue capacity.
            queue:enqueue({ event = "c" })
        end
        return { ok = true, statusCode = 200 }
    end,
}
local logger = { warn = function() end, error = function() end }
queue = EventQueue.new({
    config = { projectKey = "phc_test", maxQueueSize = 2, maxBatchSize = 2, flushAt = 2 },
    transport = transport,
    logger = logger,
    retryPolicy = RetryPolicy,
    clock = { seconds = function() return 0 end, iso = function() return "now" end },
})
queue:enqueue({ event = "a" })
queue:enqueue({ event = "b" })
queue:flush()
print("queue sent:", table.concat(sent, ","))
print("queue remaining:", queue:count())

-- Execute the unchanged Roblox-bound FeatureFlags source with only its module
-- dependencies and transport supplied by this headless harness.
local fs = require("@lune/fs")
local luau = require("@lune/luau")
local source = fs.readFile("src/Server/FeatureFlags.luau")
local scriptRef = {
    Parent = {
        FlagCache = "FlagCache",
        Parent = { Shared = { Defaults = "Defaults", Version = "Version" } },
    },
}
local modules = {
    FlagCache = FlagCache,
    Defaults = { EVENTS = { FEATURE_FLAG_CALLED = "$feature_flag_called" } },
    Version = { LibraryName = "posthog-roblox", SdkVersion = "test" },
}
local loadModule = luau.load(source, {
    environment = {
        script = scriptRef,
        require = function(name) return modules[name] end,
    },
})
local FeatureFlags = loadModule()
local firstCalls = 0
local flags = FeatureFlags.new({
    config = { projectKey = "phc_test", sendDefaultPersonPropertiesForFlags = false },
    transport = {
        fetchFlags = function()
            firstCalls += 1
            if firstCalls == 1 then
                coroutine.yield()
                return { ok = true, statusCode = 200, body = { featureFlags = { tier = "old" } } }
            end
            return { ok = true, statusCode = 200, body = { featureFlags = { tier = "new" } } }
        end,
    },
    logger = { debug = function() end, warn = function() end },
    capture = function() end,
})
local playerState = {
    distinctId = "123",
    personPropertiesForFlags = {},
    groupPropertiesForFlags = {},
    flagCalled = {},
    groupsOrNil = function() return nil end,
}
local oldRequest = coroutine.create(function() flags:reload(playerState) end)
assert(coroutine.resume(oldRequest))
flags:reload(playerState)
print("flags after newer request:", flags:getValue(playerState, "tier"))
assert(coroutine.resume(oldRequest))
print("flags after older request completes:", flags:getValue(playerState, "tier"))

-- PostHog:Group stores membership on PlayerState and puts groupProperties only
-- in the $groupidentify event. It never calls setGroupProperties.
local groupState = {
    distinctId = "123",
    personPropertiesForFlags = {},
    groupPropertiesForFlags = {},
    groupsOrNil = function() return { guild = "guild_42" } end,
}
local body = flags:_buildBody(groupState)
print("Group-derived flags membership:", tostring(body["$groups"].guild))
print("Group-derived flags properties:", tostring(body.group_properties))
