-- Helpers for Campfire. Only functions may be defined at the top level:
-- story state lives in vars and on objects, so it's saved and undone.

-- GATHER BRANCHES: adds an armful to the firewood the player carries.
function gather_wood()
  local wood = world.firewood
  if wood.location ~= nil and not wood:is_in(player) then
    say("You've already started a pile of firewood. Go and get it first.")
    return
  end
  -- :move_to ignores capacity, so check it ourselves.
  if not player:can_hold(wood) then
    say("Your hands are too full to carry any branches.")
    return
  end
  wood:move_to(player)
  vars.wood = vars.wood + 1
  if vars.wood == 1 then
    say("You gather an armful of dry branches.")
  else
    say("You add more branches to the pile in your arms.")
  end
end

-- Uses up some wood, and puts the firewood away once there's none left.
function burn_wood(n)
  vars.wood = vars.wood - n
  if vars.wood <= 0 then
    vars.wood = 0
    world.firewood:move_to(nil)
  end
end

-- How a burning fire looks, from how many actions it has left.
function fire_words()
  local left = timer_left("fire_burns")
  if left > 5 then
    return "A fire is crackling merrily in it."
  elseif left > 2 then
    return "The fire is burning lower."
  else
    return "The fire is almost out. It needs more wood."
  end
end
