# s³ 0.3 — the walkable world

This document specifies the next version. It amends `SPEC.md`; where the two differ, this one governs from 0.3 on. Section numbers here are its own.

## 0. Ground

0.1 s 1.0 is the ground of s³. Every s 1.0 program imports as a world and runs with the same output, in the interpreter and compiled. The s 1.0 corpus test stays in the suite and gates every release.

0.2 Text mode is complete. Every element of a world has an s text form: bricks, plates, shelves and boxes as in `SPEC.md` §11, and from 0.3 the graph, devices, wires, tracks, scopes and props as s lines in a `<world>.graph.s` unit (§9). A world is fully editable as text; the 3D scene is a view of the text, and the text a view of the scene. Round trip text → world → text is the identity on canonical text.

0.3 The new vocabulary of 0.3 is defined in s libraries (`std/logic.s`, `std/elec.s`, `std/datum.s`, `std/machine.s`, `std/prop.s`). A primitive is an s definition plus a 3D representation. Natives in Rust exist only where s would be too slow, and each native has a reference definition in s that the tests compare it with.

## 1. Reading key: what 0.2 is

1.1 A **brick** is one s token in one grid cell. A **column** is a vertical stack of bricks and is one line of s, read bottom to top. A **plate** is a grid surface holding columns; its **arrow** gives the order in which its columns are read. A **shelf** is a plate of definitions; its height is its generation. A **box** is a quotation `{…}` whose inside is a space of its own that can be entered.

1.2 A **sphere** is a place where code runs on an event. It has a body (a plate of bricks inside it), a kind (`src`, `fn`, `ratchet`, `out`, `gate`), a position and an aim. A **pulse** is an event: a sphere receives a line (an s stack), runs its body on it, and sends what is left on the line along its links.

1.3 A **link** is made by aiming: a sphere's ray strikes the nearest accepting sphere. A **sticky** link is fixed when it is made and stays as the spheres move. A **ray** link is recomputed at every emission from the current geometry, so moving a sphere from code reroutes the flow. When nothing moves, sticky and ray links behave the same.

1.4 A **ratchet** sends its line out its loop links n times, then once out its exit links. An **out** sphere shows the line it receives. A **src** sphere starts pulses, at program start and on clock ticks.

1.5 A **zone** is a box region that runs as its own process with captured names (copy, move, ref, mut) and **gates** on its walls. Zones decide where code runs: same thread, own thread, another machine, or as a remote call.

1.6 Bricks and spheres relate as code and the place it runs: a sphere is a function whose body is bricks, and links are the calls between functions, made by geometry instead of names. A sphere is a subassembly, as in Logic World: a whole script as one block with an inside and connections outside. 0.3 keeps the floating sphere graph as a layer of the world and adds a grounded layer of devices, wires and beams on the grid (§3). The two layers connect, and a sphere moves between them (§3.9).

## 2. Modes and movement

2.1 The scene is an unbounded flat ground plane in a dark void lit by a key light and a fill light. Grids are painted on the ground where they are needed: under plates, around devices, and within a radius of the player, fading out with distance.

2.2 **Walk mode** is first person. W A S D move, the mouse looks, Space jumps, Shift sprints, Ctrl crouches, C slides while sprinting. Gravity holds the player to the ground, to plates and to the tops of blocks. The player steps up half a cell without jumping and climbs a full cell edge with a vault. Movement has acceleration and momentum tuned for flow: the feel is Mirror's Edge, Descenders and Watch Dogs 2 driving, moving through an environment, not an RTS view of it.

2.3 **Fly mode** is the 0.2 camera. F toggles between walk and fly. **Orbit** (Tab) stays.

2.4 **Mouse mode** frees the cursor without leaving walk or fly: Alt held, or the backtick console open, or a window under the pointer. In mouse mode the scene keeps rendering and clicks go to windows and to picked objects.

2.5 **Ride** follows a wire or a track: aiming at it and pressing R rides it at the speed of its traffic, so a data path is followed as a tube. Ride on a wire follows the next pulse along it; ride on a track follows an item.

2.6 **Crosshair interaction.** A ray from the screen center picks. Left click uses the equipped item. Right click is its second use (remove, or rotate a quarter turn for devices). E picks up a movable object and carries it in front of the player; E again sets it down on the grid cell it hovers. Q drops it where it is.

2.7 **Hotbar.** Equippables stand in a vertical bar on the right side of the screen. The mouse wheel and the middle button cycle through them, number keys pick one directly. Equippables are: brick (with the word to place, typed with Enter), wire, beam, track, every device class in use (§3), datum cube, scope paint (§4), inspector, tracer, remover. The bar holds what the player has packed; the full catalogue opens in a window and items are dragged from it into the bar. Each language artifact is packable: a script plate, a box or a module can be picked up as one item and placed elsewhere.

## 3. Primitives

3.1 A **device** is a block of one or more cells standing on the grid, with typed ports on its faces. Every device class is an s definition `{body} 'name' device.` plus a model reference and a port list. The 0.2 sphere kinds are also device classes: `src`, `fn`, `ratchet`, `out`, `gate`.

3.2 A **wire** connects an output port to an input port along grid cells and carries values at event time: a value sent into a wire arrives in the same pulse. A wire is the 0.2 sticky link. A **beam** is the 0.2 ray link: a straight emitter ray that connects to whatever device it currently strikes. Wires are laid cell to cell with the wire tool, drawn as thin tubes on the ground or in the air, and can cross under or over each other.

3.3 A **track** carries items in world time: an item moves along it at a fixed number of cells per tick and can queue. Tracks are conveyors. Wires carry values within a step; tracks carry things across steps, and that difference is what the two are for.

3.4 **Logic.** `std/logic.s` defines `and or not xor nand nor buf`, `latch` (SR), `dff` (D flip-flop), `clock`, `button`, `switch`, `lamp`, `seg7`. In a logic scope (§4) wires carry bits and the scope advances by ticks: each gate reads its inputs from the previous tick and writes its output for the next, so every gate is one tick of delay and loops settle or oscillate as in hardware. A device's inputs are the outputs of the devices wired to it, in link order. Every output starts at 0, or at the device's `init` param. A source is a switch that puts out its `on` param, or a square wave when it has `every`; an out device is a lamp that shows its input a tick later; any other device runs its body on its inputs and puts out the line it leaves.

3.5 **Electronics.** `std/elec.s` defines `res cap ind led diode npn pnp src ground` with their parameters as s records. An electronics scope solves its circuit each tick by modified nodal analysis with piecewise-linear models for diodes and transistors. The solver is a native word `mna` with a reference implementation in s that the tests run on small circuits.

3.6 **Data.** A **datum cube** holds 24 bits, three bytes, red green blue. Each face carries a 2 × 2 grid of tiles, 6 faces × 4 tiles = 24 tiles, one per bit: +x and −x are the high and low nibble of red, +y and −y of green, +z and −z of blue. A tile is black for 0 and white for 1, and clicking it toggles the bit. Seen from outside each face, tile (u, v) has u to the right and v downward (on ±y, north is up), and tile `2v + u` carries bit `3 − (2v + u)` of the face's nibble, so the top-left tile is the nibble's highest bit; `tile_bit(face, u, v)` in `core/src/world.rs` is the normative mapping. A datum stands on its cell's floor: its frame spans x and z from −½ to ½ and y from 0 to 1 about the center of the floor, and scale and rotation pivot there. A **byte bar** is 8 tiles in a row; a **word block** is 32 or 64 tiles.

`std/datum.s` gives the bytes and bits of a value as words: `red green blue` (v → byte), `rgb` (r g b → v), `bitof` (v n → bit n), `flip` (v n → v with bit n toggled).

3.6.1 **Gimbal.** A datum is scaled and rotated in place with the gimbal, an equippable that works as Blender's transform gizmo: three axis handles and three plane handles for scale, three rings for rotation, a center handle for uniform scale. Holding X, Y or Z constrains to that axis; Ctrl snaps scale to sixteenths of a cell and rotation to 15°; typing a number while dragging sets the amount exactly. A scaled datum is a rectangular block, so datums build walls, slabs, beams and panels and give the world its detail.

3.6.2 **Mutable schema.** A datum's schema is its value plus the transform parts it has. Scale (`sx sy sz`) is part of the schema when it differs from 1 1 1, and orientation (`rx ry rz`, degrees, applied in z y x order) when it differs from the standard orientation. A part that is absent reads as its default and costs nothing; setting a part, from the gimbal or from code, adds it, and setting it back to its default removes it. Each present part is a name of its own, `c7/sx` `c7/sy` `c7/sz` `c7/rx` `c7/ry` `c7/rz` beside `c7` for the value, read and written from s like any name, and `c7 schema` puts the names of the parts present on the line. Code that writes a part moves the cube in the scene at the next report. The bits stay on their faces in the datum's own frame, so rotating a datum changes which face shows which nibble in the world and leaves its value as it was.

3.6.3 **Memory tender.** A datum is detail in the world and memory for programs at once: a wall built from scaled datums is also a set of numbers and transforms that code reads and writes, and code that writes them moves, stretches and recolors the wall. Machines treat the transform parts as data: a `rotator` writes `ry`, a `painter` writes the value, a `stretcher` writes the scale parts, and a `reader` puts the whole schema on a wire as a list.

3.7 **Machines** (`std/machine.s`): `conveyor`, `rotator` (turns an item a quarter), `splitter`, `merger`, `cutter` (a cube into three byte bars), `stacker` (three byte bars into a cube), `painter` (applies a mask to an item), `reader` (an item becomes a value on a wire), `writer` (a value on a wire becomes an item on a track), `bin`. Machines take items from tracks and hand items on, as in Shapez 2.

3.8 **Props** (`std/prop.s`) are glTF models with named animation states. Code sets a prop's state, position, rotation and scale; the prop blends to it. Props hold no logic.

3.9 **Two layers.** The world has a grid layer (devices, wires, beams, tracks on the ground and on plates) and a sphere layer (spheres floating above, linked by sticky and ray links as in 0.2, moving freely, raycasting recursively with constant-space tail calls). 0.2 worlds load into the sphere layer unchanged. A sphere and a device are the same thing in two placements: setting a sphere down on a grid cell makes it a device of the same kind with the same body and links, its sticky links becoming wires and its ray links beams; lifting a device off the grid makes it a sphere again. A **mast** is a device that stands a link up from the grid to the sphere layer, so a wire on the ground can feed a floating sphere and a sphere can drive a lamp on the ground.

3.10 **Assemblies.** Any selection becomes one block: a script plate, a box, a sphere with its body, or a set of devices with the wires between them. Its open connections become the block's ports, named after the wires they cut. An assembly is entered like a box, carried and packed into the hotbar as one item, placed many times, and edited once for all its placements. A sphere is an assembly whose inside is its body plate. Assemblies are the in-world form of modules (§6): saving an assembly as a world makes it a module.

## 4. Scopes

4.1 A **scope** is a region where a set of rules applies: a dialect of word definitions and an evaluation rule. A scope is a zone of kind `scope` with the param `dialect`; it runs on its parent's thread, loads its dialect's library when it spawns, and spawns when its parent's main plate is done or at the first tick. Scopes are painted on the ground with the scope tool as rectangles of cells extending upward.

4.2 Dialects in 0.3: `s` (plain), `logic` (ticked bits, §3.4), `elec` (§3.5), `lisp` (`std/lisp.s`), `forth` (`std/forth.s`), `apl` (`std/apl.s`), `machine` (items on tracks). A world may define further dialects as s libraries.

4.3 Names resolve innermost scope first, then the enclosing scopes, then the world's globals, then s. A definition made in a scope is visible in it and in scopes nested in it, and nowhere else. A dialect can overload any word, including commands, inside its scope only. Global scope pollution does not happen: two scopes can give `+` different meanings side by side.

4.4 Values cross a scope boundary only through a **port**: a gate on the scope's wall whose body is its conversion. A port's body runs in the scope, in the scope's words, in both directions. Into a ticked scope a crossing sets the port's output; out of it, the port sends what reaches it whenever that changes. The standard conversions are bit ↔ number (0 and 1), byte bar ↔ number, cube ↔ number, item ↔ value (reader and writer), line ↔ list.

4.5 Scopes and zones are independent: a zone says where and how code runs, a scope says what words mean. A region can be both.

4.6 Text form: each scope exports as its own unit file `<world>.<scope>.s` that begins with the line `'<dialect>' dialect.`, so a lisp section reads as lisp text and a logic section as gate declarations.

## 5. Qualitative dubbing and plate memory

5.1 **Dubbing** binds a value type to an appearance at each level of detail. A dub is an s definition `{near far} 'type' dub.` naming the rendering near and far.

5.2 The datum cube's dub: within 12 cells it is the tiled cube of §3.6; beyond that it is a single flat square of the color its three bytes make, so a field of cubes seen from afar is an image, and walking into it turns pixels into cubes. A scaled or rotated datum keeps its transform in both renderings: near, its tiles stretch with its faces; far, it is a flat rectangle of its color with its scaled footprint and orientation.

5.3 **Units of plate memory** are the in-scene things code animates: datum cubes, items on tracks, props, lamps, displays. They are memory with an appearance, there to be enjoyed in the scene. Every unit of plate memory has a name and a value, readable and writable from s like any name, and a dub that renders it.

5.4 **Mutation log.** Every toggle, set or move of a unit of plate memory made by the player is recorded as s text that does the same thing: turning bit 23 of cube `c7` on records `8388608 c7;` and turning it off `8388608 c7:`, setting the whole value records `16753920 c7#`, and a gimbal drag records the parts it set, as `2.5 c7/sx#` or `45 c7/ry#`. A log starts with the cube's value when it was made. The log of a unit is a program that rebuilds its state, and it can be copied into a column, edited and replayed. A log longer than 256 lines folds into the shortest program that rebuilds the unit: its value, then each present part.

## 6. Compiled modules

6.1 A **module** is a world saved with an export list: gates, ports, scopes and props it offers. Placing a module in another world places one block with the module's ports.

6.2 A module whose spec has a compiled build in the cache (keyed by the spec hash, `SPEC.md` §10.11 as amended in 0.2) runs compiled in the background, as a partition on a thread of the server or on a host. Its exported gates are called as coupled rpc gates; its props and units of plate memory stream state deltas back to the studio, which renders them. A module without a build runs interpreted and is built in the background; the switch to the compiled build happens at an event boundary, as live edits do.

6.3 Modules import other modules. Resources (models, textures, sounds) travel with a module in its folder and are referenced by relative path.

## 7. Parallelism from the grid

7.1 Each column has a read set and a write set of names. Caps make them static: `x~` and `x` reads, `x#` `x;` `x:` writes. A column that calls a word whose effects are not static has the effects of that word's definition, computed transitively; a word with dynamic effects (eval of runtime text, `~` of a computed name) makes its column a barrier.

7.2 Two columns are **independent** when neither writes a name the other reads or writes, and neither is a barrier or does I/O. The columns of a plate form a dependency order; any reordering that keeps it gives the same result.

7.3 The studio shows the order: columns are tinted by independent group, and selecting a column draws arrows to the columns it depends on. Reordering columns by drag is refused where it breaks the order, and offered where it shortens the critical path.

7.4 The compiler runs independent groups as tasks on a pool of threads, with the same output as the sequential program. A repeat whose body's iterations are independent becomes a data-parallel loop, and arithmetic over lists of numbers compiles to fixed-width lane loops (`[f64; 4]` chunks) that the Rust compiler vectorizes. Every parallel form has a sequential twin, and the tests compare them on random inputs.

## 8. Windows and themes

8.1 Every panel is a window: palette, inspector, output, minimap, hotbar catalogue, the top bar. A window has a title strip. Dragging the strip moves it; double-clicking it collapses it to the strip; windows snap to the screen edges and to each other.

8.2 A window docked at the bottom edge flips vertically: its strip goes to the bottom and its content order reverses, so the strip stays at the edge it is docked to. The top bar docks top or bottom the same way.

8.3 Windows keep their size, except the output, whose height is set by dragging its free edge. The output is also the console: backtick drops it from its docked edge over the scene, with an input line that runs s in the running session, with history and completion.

8.4 Layout, theme and hotbar are saved per user (per browser without accounts, per account with them).

8.5 Themes are token sets for the interface and the scene. Every theme keeps square corners and box shadows on windows.

| token | milk | monokai pink | zen |
|---|---|---|---|
| window | `#fbf6ee` | `#241d24` | `#ffffff` |
| ground | `#e9dbc6` | `#120e12` | `#f4f4f4` |
| ink | `#3b2d20` | `#f6eef3` | `#000000` |
| ink2 | `#6d5a47` | `#a898a4` | `#5a5a5a` |
| line | `#d9c6ab` | `#3a2f3a` | `#000000` |
| accent | `#7a5532` | `#ff5fa2` | `#000000` |
| number | `#2f7d73` | `#ae81ff` | `#000000` |
| command | `#8a4b2a` | `#ff5fa2` | `#000000` |
| quote | `#a07a2c` | `#e6db74` | `#5a5a5a` |
| name | `#3b5f9a` | `#66d9ef` | `#000000` |
| definition | `#7b3f8c` | `#f78fb3` | `#000000` |
| error | `#b0413e` | `#ff6b6b` | `#000000` |
| sky | `#e9dbc6` | `#000000` | `#ffffff` |

In zen, syntax is carried by weight and underline instead of hue: commands bold, definitions underlined, quotes in the second ink. Walk mode in monokai pink is the black void of §2.1; in zen it is a white void with black edges.

## 9. Text form of the graph

9.1 `<world>.graph.s` holds one s line per sphere, device, link, wire, beam, track, scope, prop and assembly, in id order:
- `'add' 'fn' 0 4 6 1 0 0 device.` — name, class, position, facing;
- `'add' 'out' 'show' 'in' wire.` — from device and port, to device and port, optional `cells` list for the route;
- `'lens' 'beam' 60 beam.` — emitter and reach;
- `'belt' 1 0 1 0 1 1 0 … track.` — name, speed in cells per tick, then x y z of each cell from start to end; consecutive cells differ by one along one axis;
- `'logic' 0 0 8 8 'gates' scope.` — dialect, rectangle, name;
- `'door' 'door.glb' 3 0 3 prop.`;
- `'c7' 3 1 4 16753920 datum.` — name, cell, value; then `'c7' 'sx' 2.5 part.` for each present part and `'c7' {…} log.` with the mutation log.

9.2 Loading a world runs its graph unit in a dialect where these words build the graph instead of running anything. Editing the graph unit as text and saving rebuilds the scene; editing the scene rewrites the unit.

## 10. Tutorial

10.1 The Levels button leaves the top bar. A menu (≡ in the top bar, Esc in walk mode) holds: worlds, new, import and export, tutorial, theme, keys, account.

10.2 **Tutorial mode** is a sequence of chapters. Each chapter is a world with a goal, as levels are now, plus guide steps: each step names an event (a brick placed, a run started, a cube toggled, a wire laid) and shows its text until that event happens. Stars and progress are kept per user.

10.3 Chapters: the ten 0.2 levels; walking and the hotbar; a gate circuit (a lamp lit by an and gate); datum cubes (paint a color by toggling tiles, then see it as a pixel from afar); a conveyor line (cut a cube into bytes and stack it back); scopes (a lisp section feeding a logic section through a port); a module (compile a world and use it as a block).

## 11. Multiplayer

11.1 **Accounts.** The server keeps users with a name and a password hash (Argon2id). Sessions are tokens in a cookie. Without `--accounts` the server has one local user and binds to 127.0.0.1, as in 0.2.

11.2 **Permissions.** Each world has an owner and grants roles: viewer, editor, runner, admin. Permissions are checked per mutation class: bricks and plates, devices and wires, zones and scopes, params, running, building, and words that reach the host (`<` shell, files, `tcp`). Host-reaching words run only for admins. A grant can be limited to a scope region, so a player builds only inside the area given to them.

11.3 **Diff as you go.** Every edit is an op with its inverse (as in 0.2). The server's core thread orders all ops of a world, gives each a sequence number, applies it and broadcasts it over a WebSocket to every client of that world. A client sends ops tagged with the sequence number it had seen; the server applies an op whose touched ids are untouched since that number, and refuses an op that conflicts, sending the current records of those ids. Other players appear as avatars with their aim ray and selection.

11.4 **Sidecar log.** Beside `<world>.json` the server keeps `<world>.log`, append-only, one JSON line per applied op: `{"seq","time","user","op","inverse","hash"}`, where `hash` is FNV-1a 64 of the world after the op. Every 1000 ops it writes `<world>.<seq>.snap.json`. The world at any sequence number is the nearest snapshot before it plus the log replayed. The log is never rewritten.

11.5 **Versioning on the log.** A tag names a sequence number. Branching a world copies the snapshot and starts a new log that records its origin `world@seq`. The studio has a history scrubber that shows the world at any sequence number and a blame view that shows, for each brick and device, the user and sequence number of its last write. Undo for a player applies the inverses of that player's own ops, newest first, while they still apply.

## 12. Distribution

12.1 The code is dual licensed MIT and Apache-2.0, as is usual in Rust.

12.2 Release builds are single executables for Linux, Windows and macOS: `s3-studio` with the studio files embedded, opening the browser at start, and `s3`. The itch.io page offers them free, with the tutorial as the way in; the GitHub repository carries the source, the specs and the tests.

12.3 The Bevy studio of `BEVY.md` stays the later target; 0.3 keeps the Three.js studio and makes every 0.3 element a record and an op the Bevy studio can take over unchanged.

## 13. Order of work

1. Text form of the graph (§9), the grid layer of devices, wires and beams beside the sphere layer, setting down and lifting (§3.9), masts, and assemblies (§3.10).
2. Window system, themes, menu and tutorial mode (§8, §10).
3. Walk mode, crosshair interaction, hotbar, ride (§2).
4. Scopes and ports (§4), then `std/logic.s` with ticked evaluation (§3.4).
5. Datum cubes, the gimbal and the mutable schema, dubbing, mutation log (§3.6, §5).
6. Tracks and machines (§3.3, §3.7), props (§3.8).
7. Modules and the background compiled fast path (§6).
8. Dependency analysis, the studio view of it, parallel compilation (§7).
9. Accounts, permissions, diff as you go, sidecar log (§11).
10. Electronics (§3.5).
11. Release builds and the itch.io page (§12).

## 13a. Tracks and machines as built

13a.1 Items advance front first by `speed` cells per tick, each stopping one cell behind the item ahead. A front item on a track's last cell moves to the cell past the end: into the machine standing there if it accepts, or onto the start of a track that begins there if that cell is free.

13a.2 A machine is a device whose body is exactly one machine word. It accepts while fewer than three items wait in it; a stacker accepts a bar only for a channel it does not hold. Each tick a machine puts its first waiting item onto the start of one track beginning beside it that does not end at it; a splitter takes the outgoing tracks in turn.

13a.3 Work: rotator turns a cube a quarter about +y (blue = old red, red = old blue with nibbles swapped); cutter turns a cube into bars red, green, blue in that order; stacker turns three bars into a cube; painter sets a cube to `v & and | or` from its params; reader puts the item's value on its out links; writer turns each value of an arriving line into an item (whole numbers 0…16777215 are cubes, the rest text); bin keeps nothing.

13a.4 A world with a track runs on the clock. Items are run state; a live edit keeps the items of every track whose cells did not change.

## 14. Tests

| test | checks |
|---|---|
| `s10-corpus` | unchanged from 0.2: every s 1.0 case gives the same output imported, compiled and exported |
| `graph-text` | world → `.graph.s` → world is the identity on records |
| `layers` | every 0.2 world loads into the sphere layer with the same traces; setting every sphere down onto the grid and lifting it back gives the same traces and records |
| `mast` | a ground wire drives a floating sphere and a sphere drives a ground lamp |
| `assembly` | collapsing a selection into a block keeps the program's output; editing the assembly changes every placement |
| `wire-beam` | a wire keeps its connection while devices move; a beam reconnects to what it strikes |
| `scope-resolve` | inner scope, outer scope, globals, s, in that order; a scope's definitions are not visible outside it |
| `scope-overload` | two sibling scopes give `+` different meanings at once |
| `port-convert` | each standard conversion in both directions |
| `logic-tick` | gate delays of one tick; an SR latch holds; a ring of three inverters started with one output high oscillates with period 6 |
| `cube-bits` | each of the 24 tiles toggles its own bit; face and nibble order as §3.6 |
| `dub-lod` | the cube renders as tiles inside 12 cells and as its color outside |
| `mutation-log` | replaying a cube's log rebuilds its value and its transform parts |
| `datum-schema` | absent parts read as defaults and are not stored; setting a part adds it, setting its default removes it; `schema` lists exactly the present parts |
| `datum-frame` | rotating a datum leaves its value unchanged and moves its nibbles with their faces |
| `gimbal` | axis constraint, snapping and typed amounts give the exact parts in the record |
| `track-time` | items advance a fixed number of cells per tick and queue without overlap |
| `machines` | cutter then stacker is the identity on cubes |
| `module-fast-path` | a module gives the same results interpreted and compiled, and switches at an event boundary |
| `deps` | read and write sets from caps and transitive definitions; barriers for dynamic words |
| `reorder` | every reordering that keeps the dependency order gives the same output on the corpus |
| `parallel-twin` | each parallel form equals its sequential twin on random inputs |
| `mna` | the native solver matches the s reference on divider, RC and diode circuits |
| `perm` | each mutation class is refused without its role; scope-limited grants hold at the boundary |
| `diff-conflict` | disjoint concurrent ops both apply; conflicting ones are refused with the current records |
| `log-replay` | snapshot plus log equals the live world at every sequence number, by hash |
| `windows` | dock, flip, collapse and the console's height survive a reload |
