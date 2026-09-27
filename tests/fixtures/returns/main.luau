local value = 42
local function choose(a, b)
    return a and b or false
end
return {
    value = value,
    choose = choose,
    run = function(x)
        if x then return 1 else return 2 end
    end,
}
