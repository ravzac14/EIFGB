-- EIFGB Lua prelude. TRUSTED code: it runs with the real globals and
-- builds the sandboxed environment that author scripts run in.
--
-- `H` is a table of host primitives. The Rust side refills it before
-- every call with functions that borrow the live world, and those
-- functions become invalid as soon as the call returns. Author code
-- never sees `H` directly: everything goes through the proxies below.

local H, exhausted = ...

local setmetatable, rawset, type, tostring, error, select, pairs, ipairs =
      setmetatable, rawset, type, tostring, error, select, pairs, ipairs
local concat = table.concat

---------------------------------------------------------------- entities
-- Each entity has exactly one proxy table, so `==` compares identity:
-- `args.tool == world.oil_can` works as authors expect.

local proxies, ids = {}, {}
local Entity, EntityMeta = {}, {}

local function ent(id)
  if id == nil then return nil end
  local p = proxies[id]
  if p == nil then
    p = setmetatable({}, EntityMeta)
    proxies[id] = p
    ids[p] = id
  end
  return p
end

local function id_of(v)
  if type(v) == "table" then return ids[v] end
  return nil
end

local function need_entity(v, what)
  local id = id_of(v)
  if id == nil then error(what .. " must be an object, not a " .. type(v), 3) end
  return id
end

local function wrap(v, is_entity)
  if is_entity then return ent(v) end
  return v
end

EntityMeta.__index = function(p, k)
  local m = Entity[k]
  if m ~= nil then return m end
  return wrap(H.ent_get(ids[p], k))
end
EntityMeta.__newindex = function(p, k, v)
  if k == "location" then error("'location' can't be changed; use :move_to(place)", 2) end
  if Entity[k] ~= nil then
    error("'" .. tostring(k) .. "' is a method name, so it can't be used as a state field", 2)
  end
  if id_of(v) then
    error("can't store an object in state; store its key instead (" .. tostring(k) .. " = x.key)", 2)
  end
  H.ent_set(ids[p], k, v)
end
EntityMeta.__tostring = function(p) return (H.ent_get(ids[p], "name")) end
EntityMeta.__metatable = "locked"

function Entity:move_to(dest)
  local id = need_entity(self, "the thing to move")
  if dest == nil then H.move(id, nil) else H.move(id, need_entity(dest, "move_to's destination")) end
end

function Entity:is_in(holder)
  return H.is_within(need_entity(self, "the thing"), need_entity(holder, "is_in's container"))
end

function Entity:contents()
  local out = {}
  for i, id in ipairs(H.contents(need_entity(self, "the container"))) do out[i] = ent(id) end
  return out
end

function Entity:has(trait)
  return H.has_trait(need_entity(self, "the thing"), trait)
end

---------------------------------------------------------------- sandbox

local env = {}
local loading = false

-- Returned by block(): the engine stops the default behaviour when a
-- before_ handler returns it.
local BLOCK = setmetatable({}, { __tostring = function() return "" end, __metatable = "locked" })

local function join(...)
  local parts = {}
  for i = 1, select("#", ...) do parts[i] = tostring((select(i, ...))) end
  return concat(parts)
end

local function locked(what)
  return function() error(what, 2) end
end

env.world = setmetatable({}, {
  __index = function(_, k)
    local id = H.lookup(k)
    if id == nil then error("there is no object called '" .. tostring(k) .. "'", 2) end
    return ent(id)
  end,
  __newindex = locked("world can't be assigned to; change an object's fields instead"),
  __metatable = "locked",
})

env.vars = setmetatable({}, {
  __index = function(_, k) return H.var_get(k) end,
  __newindex = function(_, k, v)
    if id_of(v) then error("can't store an object in vars; store its key instead", 2) end
    H.var_set(k, v)
  end,
  __metatable = "locked",
})

env.game = setmetatable({}, {
  __index = function(_, k) return H.game_get(k) end,
  __newindex = locked("game is read-only"),
  __metatable = "locked",
})

env.player = ent(0)

function env.say(...) H.say(join(...)) end
env.print = env.say

function env.block(...)
  if select("#", ...) > 0 then H.say(join(...)) end
  return BLOCK
end

function env.score(n) H.score(n) end
function env.end_game(key) H.end_game(key) end
function env.open_choice(key) H.open_choice(key) end

function env.random(a, b)
  if b == nil then a, b = 1, a end
  return H.random(a, b)
end

for _, name in ipairs({
  "assert", "error", "ipairs", "next", "pairs", "select",
  "tonumber", "tostring", "type", "rawequal", "rawlen",
}) do
  env[name] = _G[name]
end

-- pcall and xpcall are wrapped so that they can't swallow the "ran too
-- long" error: once the budget is spent, the error is raised again
-- outside the protected call.
local real_pcall, real_xpcall = pcall, xpcall
local TOO_LONG = "the script ran too long and was stopped (is there an endless loop?)"
local function guard(...)
  if exhausted() then error(TOO_LONG, 0) end
  return ...
end
function env.pcall(f, ...) return guard(real_pcall(f, ...)) end
function env.xpcall(f, handler, ...) return guard(real_xpcall(f, handler, ...)) end

local function copy(t, skip)
  local c = {}
  for k, v in pairs(t) do
    if not skip[k] then c[k] = v end
  end
  return c
end

-- string.dump is removed from the real table too, because the string
-- metatable (`("x"):rep(3)`) points at it.
string.dump = nil
env.string = copy(string, {})
env.table = copy(table, {})
-- Randomness must come from the engine, so that it's saved and undone.
env.math = copy(math, { random = true, randomseed = true })

setmetatable(env, {
  __index = function(_, k)
    if k == "here" then return ent(H.here()) end
    error("unknown name '" .. tostring(k) .. "'", 2)
  end,
  __newindex = function(t, k, v)
    if loading then
      if type(v) == "function" then rawset(t, k, v) return end
      error("script files may only define functions at the top level; keep story state in vars ('"
        .. tostring(k) .. "')", 2)
    end
    error("can't create global '" .. tostring(k) .. "'; use 'local', or keep story state in vars", 2)
  end,
  __metatable = "locked",
})

return {
  env = env,
  ent = ent,
  id_of = id_of,
  block = BLOCK,
  set_loading = function(b) loading = b end,
  install = function(module)
    for k, v in pairs(module) do rawset(env, k, v) end
  end,
}
