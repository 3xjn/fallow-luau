-- Standalone review reproductions against unchanged production modules.
local FlagCache = require("../src/Server/FlagCache")
local EventQueue = require("../src/Server/EventQueue")
local RetryPolicy = require("../src/Shared/RetryPolicy")

local logger = { warn = function() end, error = function() end }
local now = 0
local clock = {
    seconds = function()
        return now
    end,
    iso = function()
        return "now"
    end,
}
local function event(name)
    return { event = name, uuid = name, distinct_id = "player", properties = {} }
end
local function queue(config, transport)
    return EventQueue.new({
        config = config,
        transport = transport,
        logger = logger,
        retryPolicy = RetryPolicy,
        clock = clock,
    })
end

local store = FlagCache.empty()
FlagCache.applyResponse(
    store,
    { featureFlags = { old = true }, featureFlagPayloads = { old = { version = 1 } } }
)
FlagCache.applyResponse(store, { featureFlags = {}, featureFlagPayloads = {} })
print("flag reload removes absent flag: expected nil, actual", FlagCache.getValue(store, "old"))
print(
    "flag reload removes absent payload: expected nil, actual",
    FlagCache.getPayload(store, "old") and FlagCache.getPayload(store, "old").version
)

local attempts = 0
local tooLarge = queue({ projectKey = "key", maxQueueSize = 4, maxBatchSize = 2, flushAt = 1 }, {
    sendBatch = function()
        attempts += 1
        return { ok = false, statusCode = 413 }
    end,
})
tooLarge:enqueue(event("oversized"))
for _ = 1, 3 do
    tooLarge:flush()
    now += 100
end
print(
    "single-event 413: attempts",
    attempts,
    "queued",
    tooLarge:count(),
    "batch size",
    tooLarge._adjustedBatchSize
)

local sent = {}
local inFlight = queue({ projectKey = "key", maxQueueSize = 2, maxBatchSize = 2, flushAt = 1 }, {
    sendBatch = function(_, payload)
        for _, item in payload.batch do
            table.insert(sent, item.event)
        end
        coroutine.yield()
        return { ok = true, statusCode = 200 }
    end,
})
inFlight:enqueue(event("a"))
inFlight:enqueue(event("b"))
local worker = coroutine.create(function()
    inFlight:flush()
end)
assert(coroutine.resume(worker))
inFlight:enqueue(event("c"))
assert(coroutine.resume(worker))
local remaining = {}
for _, item in inFlight._events do
    table.insert(remaining, item.event)
end
print(
    "in-flight overflow: sent",
    table.concat(sent, ","),
    "remaining",
    table.concat(remaining, ","),
    "expected remaining c"
)
