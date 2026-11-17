-- English text helpers available to templates and scripts.
-- Each language supplies its own version of these functions, so
-- messages never build grammar out of string pieces.

local id_of = ...
local M = {}

-- Returns the name, and whether it should be used without an article.
local function parts(x)
  if x == nil then return "something", true end
  if type(x) ~= "table" then return tostring(x), true end
  -- The player ("yourself") and things the player named never take one.
  return tostring(x), id_of(x) == 0 or x.proper
end

local function capitalize(s)
  return (s:gsub("^%l", string.upper))
end

function M.the(x)
  local name, bare = parts(x)
  if bare then return name end
  return "the " .. name
end

function M.a(x)
  local name, bare = parts(x)
  if bare then return name end
  if name:match("^[aeiouAEIOU]") then return "an " .. name end
  return "a " .. name
end

function M.The(x) return capitalize(M.the(x)) end
function M.A(x) return capitalize(M.a(x)) end

return M
