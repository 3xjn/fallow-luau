local function first(a, b, c, d)
    if a then print(a) end
    if b then print(b) end
    if c then print(c) end
    if d then print(d) end
    return a
end
local function second(a, b, c, d)
    if a then print(a) end
    if b then print(b) end
    if c then print(c) end
    if d then print(d) end
    return b
end
return { first = first, second = second }
