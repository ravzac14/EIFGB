-- Shared helpers for the sample game. This runs in the same sandbox as
-- inline handlers. Only functions may be defined at the top level here:
-- story state belongs in `vars` or on entities, so that save, restore
-- and undo can see it.

function trust_level()
  local t = vars.keeper_trust
  if t >= 2 then return "warm" elseif t == 1 then return "wary" else return "cold" end
end
