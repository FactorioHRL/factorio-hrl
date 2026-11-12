# probe-fluid-boxes

A small Factorio 2.1 mod that places machines in various orientations, with mirroring (`mirror`) and recipes, then writes to `script-output/probe.json` the pipe connections **reported by the game itself** (`LuaEntity::get_fluid_box_pipe_connections`). It is used to check that our connection logic (`crates/fsim-render/src/fluids.rs`) agrees with the engine. The expected values in that module's tests come from its output.

The probe places: a chemical plant (recipes `plastic-bar`, `light-oil-cracking` and no recipe, 4 directions, mirrored and not), a refinery (`basic-oil-processing`), a pipe-to-ground and a storage tank.

## Running (example: WSL with a Steam install on Windows)

```
P=/mnt/c/temp/fsim-probe
mkdir -p $P/config $P/mods/fsim-probe_0.0.1
cp tools/probe-fluid-boxes/mod/* $P/mods/fsim-probe_0.0.1/
cp tools/probe-fluid-boxes/mod-list.json $P/mods/
printf '[path]\nread-data=__PATH__executable__/../../data\nwrite-data=C:/temp/fsim-probe\n' > $P/config/config.ini

cd /mnt/c/temp
"/mnt/c/Program Files (x86)/Steam/steamapps/common/Factorio/bin/x64/factorio.exe" \
  --config C:/temp/fsim-probe/config/config.ini \
  --mod-directory C:/temp/fsim-probe/mods \
  --create C:/temp/fsim-probe/probe.zip
```

Output: `$P/script-output/probe.json`. The mod runs in `on_init` while the map is created, so no server needs to be started.

## What the probe taught us (Factorio 2.1.21)

- `mirror` flips the machine's ports about its X axis (before rotation).
- A recipe with **one** fluid on a given side (input or output) joins **all** ports of that side into one tank, so a chemical plant with `plastic-bar` has both input ports active.
- With two fluids every port gets its own fluid (in order).
- When a recipe has `fluidbox_index`, it selects the port (numbered from 1 among ports of that kind). A refinery with `basic-oil-processing` uses input 2 and output 3.
- A recipe with no fluid on a given side disables the ports of that side.
- A machine without a recipe has all ports active (unless the prototype sets `fluid_boxes_off_when_no_fluid_recipe`).
- A pipe-to-ground has one regular connection (at the back relative to the underground direction) and one `underground` connection.
