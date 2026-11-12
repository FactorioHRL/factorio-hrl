script.on_init(function()
  local s = game.surfaces[1]
  s.request_to_generate_chunks({0, 0}, 4)
  s.force_generate_chunk_requests()
  local tiles = {}
  for x = -60, 60 do for y = -60, 60 do tiles[#tiles + 1] = {name = "lab-dark-1", position = {x, y}} end end
  s.set_tiles(tiles)
  local out = {}
  local n = 0
  local function probe(name, dir, mirrored, recipe)
    n = n + 1
    local x, y = (n % 8) * 8 - 28, math.floor(n / 8) * 8 - 28
    local e = s.create_entity{name = name, position = {x + 0.5, y + 0.5}, direction = dir, force = "player"}
    if not e then out[#out + 1] = {name = name, error = "create failed"}; return end
    e.mirroring = mirrored
    if recipe then e.set_recipe(recipe) end
    local r = {name = name, dir = dir, mirror = mirrored, recipe = recipe, cx = e.position.x, cy = e.position.y, boxes = {}}
    for i = 1, e.fluids_count do
      local list = {}
      for _, c in pairs(e.get_fluid_box_pipe_connections(i)) do
        list[#list + 1] = {flow = c.flow_direction, kind = c.connection_type, pos = c.position, target = c.target_position}
      end
      r.boxes[i] = list
    end
    out[#out + 1] = r
  end
  for _, recipe in pairs({"plastic-bar", "light-oil-cracking", false}) do
    for _, dir in pairs({0, 4, 8, 12}) do
      for _, mirrored in pairs({false, true}) do
        probe("chemical-plant", dir, mirrored, recipe or nil)
      end
    end
  end
  probe("oil-refinery", 0, false, "basic-oil-processing")
  probe("oil-refinery", 0, true, "basic-oil-processing")
  probe("oil-refinery", 4, true, "basic-oil-processing")
  probe("pipe-to-ground", 0, false, nil)
  probe("pipe-to-ground", 4, false, nil)
  probe("storage-tank", 0, false, nil)
  probe("storage-tank", 4, false, nil)
  helpers.write_file("probe.json", helpers.table_to_json(out), false)
end)
