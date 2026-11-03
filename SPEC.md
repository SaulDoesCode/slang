# s³

A spatial programming system. Programs are built from bricks stacked on a grid and from spheres linked by rays, in a Three.js studio. Parts of a program can be placed in zones that run on other cores or other machines (§10). A program runs in the browser or compiles to one Rust file.

s³ is the sublation of two systems:

- **s 1.0**: a right-aligned postfix language. A line of values is consumed by the word at its right end. It has suffix sigils, definitions with generations, frames, scripts with tail calls, and a compiler to Rust.
- **the sphere graph**: spheres with kinds and code slots, sticky and ray links formed by striking, pulses along live links, ratchets for loops, and a flycam with an E panel.

## 0. Sublation

0.1 **Preserved.** Every s 1.0 program is an s³ program. Every s³ program has an exact s text form plus a graph table. The s 1.0 semantics are the value machine of s³, implemented by `s3-core` (`BEVY.md`). The sphere graph's strike, sticky, ray, mute and ratchet rules carry over unchanged (§6, §7). s 1.0 workers share nothing but messages, and zones keep that isolation (§10.2).

0.2 **Cancelled.**
- Text as the only form of a program.
- The single value carried by a sphere-graph pulse.
- Global names as the only namespace.
- The rule that a word's last character decides what it does.
- Placement order in time as the only order of definitions.

0.3 **Lifted.**
- The s line becomes a vertical column, read bottom to top, so right-aligned becomes top-aligned.
- A pulse carries a whole line.
- A sticky link is a static call; a ray link is dispatch decided by geometry at run time.
- Generations become heights: lower is older.
- Quotations become boxes with an inside.
- Locals exist per activation.
- The s 1.0 worker becomes a zone. Its messages become crossings, and its isolation becomes a capture list with Rust's modes (`copy`, `move`, `ref`, `mut`) and `give` for moving names along with a line.

## 1. Space

1.1 The world is an integer grid of cells `(x, y, z)`. `+y` is up.

1.2 A cell holds at most one **brick**. A brick is a unit cube.

1.3 **Plates** and **spheres** are not bricks. They have their own positions (§2.4, §5.1) and do not occupy cells.

## 2. Bricks, columns, plates

2.1 A **brick** has `kind`, `word` (text), `cap` (one of the sigils below, or none), and `note` (text, never executed).

| kind | color | meaning | s text |
|---|---|---|---|
| `lit` | sand | literal word | `word` |
| `num` | teal | number literal | canonical number |
| `cmd` | by command group, as in the s editor | built-in command | `word` |
| `esc` | grey rim on command color | built-in command, never replaced | `\word` |
| `name` | ink, with cap shape | name form | `word` + cap sigil |
| `read` | ink, hollow | value of a name, as data | `{~word~}` |
| `box` | brace green | quotation (§3) | `{…}` |
| `quote` | ochre | raw text | `'word'` |
| `lib` | library file color | library or mode word | `word` |
| `geo` | sphere-kind color | geometry word (§8) | `word` |

2.2 **Caps** on `name` bricks are the s sigils, each drawn as a shape:

| cap | sigil | shape |
|---|---|---|
| look | `~` | ring |
| set | `` ` `` | slot |
| num | `#` | grid |
| inc | `;` | up wedge |
| dec | `:` | down wedge |
| def | `$` | lid |
| call | `.` | cone |
| pass | `..` | double cone |
| pop | `@` | hook |
| when | `?` | green collar under the cap |
| unless | `!` | red collar under the cap |

`when` and `unless` combine with any other cap and with any `cmd` or `lit` brick. The word itself may end in any character, because the cap is separate from it. In s text a word that ends in a sigil character is written `⟨word⟩` (§11.2).

2.3 A **column** is a maximal run of bricks stacked vertically in one `(x, z)` cell on a plate. It is read bottom to top as an s line: the bottom brick is the leftmost token and the top brick is the actor.

2.4 A **plate** is a flat rectangle of size `w × d` at height `y`, with an **arrow** along one of its edges. It does not occupy cells. Columns stand on it at `y + 1` and above.

2.5 **Reading order** of a plate: rows run along the arrow and are taken in order away from the arrow's origin edge. Within a row, columns are taken along the arrow. Empty cells are skipped. This is the order of lines in a text file.

2.6 Plate kinds:

| plate | holds | s text |
|---|---|---|
| `main` | the program | the main unit |
| `script` | a named body | `{…} name$` |
| `shelf` | definitions (§4) | `{…} 'word' ><` lines |
| `inside` | the interior of a box or a sphere | the box's or sphere's text |

2.7 A plate's text is its columns in reading order, joined with newlines. A column's text is its bricks bottom to top, joined with spaces.

## 3. Boxes

3.1 A `box` brick holds an `inside` plate. In the studio, opening a box shows its inside in place, scaled into the cube, or as an overlay (§12.6).

3.2 A box's text is `{` + its inside plate's text + `}`. Boxes nest; nesting depth is brace depth.

3.3 A **socket** is a `sock` brick inside a box: its text is `~name~`. In a box at depth 1 it is the s interpolation (§1.4 of the s spec); deeper it stays text.

3.3a A box whose spacing matters (`{ }`, text with newlines or runs of spaces) keeps its text exactly as a **raw box**: the word holds the text and there is no inside plate. A token the brick kinds cannot express (`pre{fix}`) is a `raw` brick, written as typed.

3.4 A box is data until something runs it: `~`, `if`, `map`, `times`, a definition, a script, or a sphere body.

## 4. Shelves and generations

4.1 A **shelf** is a plate holding definition columns. Each column is a box brick topped by a name tag; its text is `{…} 'word' ><`.

4.2 **Height is generation.** When a program starts, shelves are processed from the lowest `y` upward. Shelves at equal `y` go in order of `(x, z)`, and within a shelf the columns go in reading order. Each column defines its word with the next generation number.

4.3 By s 1.0 §11, a body sees only definitions on shelves below its own position in this order. The studio draws that visibility: hovering a word shows a line down to the shelf brick it resolves to.

4.4 Shelves run before the main plate. A `><` column placed on the main plate keeps s's run-time ordering: it runs when its line is reached.

4.5 Replacing a definition brick replaces it in place and keeps its height. Definitions placed in the studio never accumulate. A `><` executed in a loop at run time accumulates, as in s.

## 5. Spheres

5.1 A sphere has:
- `id`, `name` (unique), `kind`, `pos` (three numbers, not snapped to the grid), `r`;
- `aim` (unit vector), `reach`, `strike` (`sticky` or `ray`);
- `params`, `muted` (a set of ids);
- `body`, an inside plate holding its code as bricks.

5.2 Kinds keep the sphere graph's colors and slots:

| kind | color | body receives | after the body |
|---|---|---|---|
| `src` | amber | an empty line, on run or clock | the remaining line is emitted on `out` |
| `fn` | blue | the arriving line | the remaining line is emitted on `out` |
| `ratchet` | rose | the arriving line; local `:i` holds the arrival count | the remaining line is emitted on `loop` while `:i < n`, else on `exit`, and the count resets |
| `out` | green | the arriving line | the line is shown on the sphere's billboard and kept in the trace |
| `gate` | slate | the arriving line, in the sending zone (§10.6) | the line crosses the wall |

5.3 A remaining line that is empty emits nothing; that branch ends.

5.4 The `emit` word (§8) sends the current line along `out` at any point in a body and leaves the line in place.

5.5 A sphere's name is also a script name. `name.` runs its body as a plain call, with no pulse and no emission.

## 6. Links

6.1 These carry over from the sphere graph without change:
- strike creates links (rule 2.3);
- the muted set and removal holding (2.4);
- sticky links are always live (3.1);
- ray links are live only while hit (3.2);
- ports: `out`, or `loop`/`exit` on a ratchet;
- at most one link for each `from`/`to` pair.

6.2 A **sticky** link is a static call edge: the body of `to` always runs on what `from` emits.

6.3 A **ray** link is dispatch decided by geometry. Whether `to` runs depends on positions and aims at the moment of emission, and code changes those through `move`, `aim` and `at` (§8). In s, a lookup `x~` decides what runs by value; in s³, a ray decides by where things are.

6.4 Ray intersection is defined by one formula, evaluated in IEEE double precision in this order, so the browser and Rust agree bit for bit:

```
d = aim, o = from.pos, c = to.pos, r = to.r
m = o - c
b = m·d
q = m·m - r*r
if q > 0 and b > 0: miss
disc = b*b - q
if disc < 0: miss
t = -b - sqrt(disc)
if t < 0: t = -b + sqrt(disc)      (origin inside: far surface)
hit iff 0 <= t <= reach
```

## 7. Pulse

7.1 A **pulse** is a stack of events `{sphere, line}`. `run(source)` pushes `{source, []}`.

7.2 A step pops one event and runs that sphere's body on a copy of its line, with a fresh activation (§9). It then refreshes the strikes (sphere graph rule 2.3). For the port chosen by §5.2, it pushes one event per live link on that port, in descending link id, so the lowest id runs first.

7.3 A run ends when the stack is empty or the step budget (`params.budget`, default 100000; 0 means no limit) is used up. Running out of budget is reported. It is not an error.

7.4 The **clock** advances one tick per frame in the studio, or per `tick` call in compiled programs. A `src` sphere with `every = k > 0` runs on every k-th tick.

7.5 An error in a body ends that branch. The error is recorded on the sphere, and the run continues with the next event.

7.6 **Constant space.** The pulse stack is the trampoline. A body has finished before the events for its emissions run, so every emission is a tail position. A chain of ray or sticky hits, a sphere whose ray reaches a sphere that reaches it back, or a ratchet looping on itself, runs in constant space for any number of hops: one event is popped and at most one pushed. No hop uses the host stack. Fan-out to k live links adds k − 1 events, and that growth is the shape of the recursion, as in a tree walk. Inside a body, `name.` in last position is an s tail call. In other positions it pushes a frame on the machine's own stack, never the host stack, so recursion is limited by memory (`BEVY.md` §2.2).

## 8. Geometry words

These are available in sphere bodies and act on the running sphere. Elsewhere they are errors. Where no graph runtime is present (a plain s program) they are ordinary words.

Graph words take their arguments from the top of the line and leave the rest of it, so the line a sphere carries passes through them.

| word | args | effect |
|---|---|---|
| `move` | dx dy dz | translate the sphere, then refresh strikes |
| `aim` | x y z | set the aim (normalised), then refresh |
| `at` | name | aim at the named sphere's center |
| `pos` | | push x y z |
| `hit` | | push the name of the sphere the ray hits, or `404` |
| `reach` | n | set the reach |
| `emit` | | send the line along `out` now (§5.4) |
| `self` | | push the sphere's name |

## 9. Activations and locals

9.1 Every run of a plate is an **activation**: the main plate, a script call, a sphere body, a box run by `~`, or a definition expanding.

9.2 A name beginning with `:` is **local**. `:x~`, `` :x` ``, `:x#`, `:x;`, `:x@` and the rest act on the current activation's own mem, numbers and lists. Tail calls replace the activation. Other calls create a new one.

9.3 A definition expanding gets its own activation for `:` names. Library temporaries written as `:` names cannot collide with the caller's names or with each other.

9.4 Names without `:` are global, as in s 1.0.

## 10. Zones

10.1 A **zone** is an axis-aligned box of cells `[min, max]` with an `id`, a `name`, a `kind` and `params`. Zones nest and do not otherwise overlap. The world is the zone `main`, id 0, kind `local`. Bricks, plates and spheres belong to the innermost zone that contains them. A plate lies wholly inside one zone.

10.2 A zone is a closure. It runs as a sequential process with its own interpreter: its own mem, numbers, lists, handles, scripts, definitions, pulse stack, clock and budget. The outside reaches it only through its **captures** (§10.5), taken when it spawns, and its **gates** (§10.6), crossed while it runs.

10.3 **Kinds.** The kind decides where a zone runs. Semantics are the same for every kind. Kinds differ in cost, timing and failure.

| kind | runs | Rust | browser | crossings travel by |
|---|---|---|---|---|
| `async` | on its parent's thread, interleaved with the other zones there | a zone on its parent's scheduler (§13.5) | the parent's worker | an in-memory queue |
| `thread` | on its own core | `std::thread`, 64 MiB stack | its own Web Worker | a channel of messages (§10.11), handles included |
| `net` | on another machine, `params.host = host:port` | the same binary started with `--serve` | WebSocket | TCP, wire messages |
| `rpc` | remote procedure coupling; `params.at` is `core` or `host:port` | as `thread` or `net` | as `thread` or `net` | call and return messages (§10.8) |

10.4 **Lifecycle.**
- A zone **spawns** at its first crossing, or when its name is given to `spawn`. Spawning sends the spawn image: the parent's definitions as they stand at that moment, the mem names ending in `.s` (so libraries are not loaded twice), and the captures. The zone then runs its own shelves, then its own main plate if it has one, and waits for crossings.
- A zone is **quiet** when its shelves and main plate have finished, its pulse stack is empty, no event is suspended, and none of its `src` spheres has a running clock.
- `join` stops the zone's clocks, waits until the zone is quiet and its inbox is empty, then ends the zone and releases its captures (§10.5). A later crossing respawns it fresh.
- The program ends when every zone is quiet and every inbox is empty, or when main's budget is used up. Every live zone is then joined, innermost first and in id order within a level.
- A zone is **lost** when its thread panics or its connection fails. A lost zone releases its captures with loss (§10.5).

10.5 **Captures.** A zone has a capture list of global names of its parent, each with a mode. A name covers everything stored under it: mem, numbers, list, script and handle. `:` locals are not captured.

| mode | Rust | inside the zone | outside while the zone is live | at join | on loss |
|---|---|---|---|---|---|
| `copy` | `let x = x.clone();` then `move` | its own copy, read and write | unchanged, read and write | nothing returns | nothing |
| `move` | `move` | owned, read and write | reading is the error `moved: x`; writing sets it fresh | nothing returns | nothing |
| `ref` | `&x` | read only; writing is the error `borrowed: x` | read only; writing is the error `lent: x` | the outside may write again | as at join |
| `mut` | `&mut x` | read and write | any use is the error `lent: x` | the zone's value moves back | the name becomes moved, with the error `zone lost: x` |

A child zone may capture its parent's captured names under these rules. From a `ref` name it may take `copy` or `ref`. From a `mut` name it may take `copy`, `ref` or `mut`. From a `copy` or `move` name it may take any mode. Any other combination is refused at spawn.

What may travel to each kind:

| stored under the name | `copy` | `move`, `mut` to `async` or `thread` | to `net` or `rpc` at a host |
|---|---|---|---|
| text, number, list, script | yes | yes | yes |
| handle | no | yes | no: `handle cannot leave host` |

Definitions always travel as a copy of their text.

10.6 **Gates.** A gate's ray points into one side along its aim and into the other side along the opposite direction; each side tests only its own candidates.
- A **gate** is a sphere of kind `gate`, colored slate, whose center lies on a wall of a zone. It has an outer side in the parent and an inner side in the zone.
- A gate does not move. `move`, `aim` and `at` on a gate are errors. Both zones know its position, so each side decides strikes on it locally.
- Links from parent spheres attach to the outer side, and links to and from inner spheres attach to the inner side. No link crosses a wall in any other way.
- A pulse that arrives at one side is a **crossing**. The gate's body runs in the sending zone on the line. The line and the given names (§10.7) are then encoded and put in the receiving zone's inbox. On delivery, the gate emits the line on the other side's `out`.
- A sphere's ray is tested against the spheres of its own zone, the gates on its own zone's walls (inner side), and the gates on its child zones' walls (outer side). No other sphere is a candidate.
- `move` that would take a sphere outside its zone is an error.

10.7 **give.** In a gate body, `x give` moves the name `x` out of the sending zone into the crossing. On delivery `x` is set in the receiving zone, replacing what was stored under it. This is a channel send: the value moves with the message. `give` outside a gate body is an error. Giving a name captured as `ref` or `mut` is the error `borrowed: x`.

10.8 **rpc zones.** Every gate on an rpc zone's walls is a call gate.
- A pulse arriving at the outer side is a **call**. The event in the parent suspends and the line crosses. The zone runs until it is quiet. The lines that arrive at the gate's inner side during the call, joined in arrival order, are the **return**. The return crosses back, and the gate emits it on its outer `out`. An empty return emits nothing.
- In a body, `gate.` with a call gate's name makes the same call in code. The current line, or the current frame, crosses; the activation suspends; and the return replaces what crossed, as `name.` does for a script.
- An rpc zone handles one call at a time. Calls from several senders queue by the merge rule (§10.9).
- A `call` message carries the chain of zones with calls pending behind it. A call into a zone already in its chain is the error `rpc reentry: name`, because that zone cannot become quiet until the call returns. Recursion across zones uses pulses, which do not wait.
- **Coupling another world.** An rpc zone with `params.peer = host:port` and `params.image = none` spawns nothing. It connects to a world running `s3 serve addr`, which offers the call gates of its rpc zones that have `params.export = true`. Calls match by gate name. The coupled zone has no captures and contains only gates. Program hashes are not compared.

10.9 **Order.**
- A zone steps one event at a time (§7.2). A blocking word suspends the current event; the zone then runs nothing else, and the other zones on the same thread run. The blocking words are `>t`, `_f` and `@f` on a connection, `tcp`, `join`, and a call (§10.8).
- A zone takes its next inbox message only when it is quiet. The message becomes an event at the gate.
- `params.merge` sets how a zone with several sender zones reads its inbox. `arrival` is the default: messages are taken in arrival order. With `round`, the zone waits for the next message from each sender in turn, in sender id order, and skips senders that have ended. A `round` zone waits forever for a live sender that never sends.
- A zone's trace is a function of the sequence of messages it takes. When every zone has one sender or uses `round`, and no zone has a clock or uses `tcp`, files or workers, every schedule and every placement give the same per-zone traces.

10.10 **Lockstep.** `--lockstep` in a compiled program, or the studio's lockstep switch, fixes one schedule. The run proceeds in rounds. In a round, each live zone in id order does one of the following: steps one event; resumes a suspended event whose reply has been delivered; or, if it is quiet, takes one message. Messages sent during a round are delivered at its end, in order of sender id and then send order. Conformance runs use lockstep.

10.11 **Wire.** A message is a sequence of netstrings `len:bytes,`, in this order:
- kind;
- zone id;
- gate name;
- sequence number;
- value count, then the values;
- given count, then for each given name: its name, its table and its value.

A list is a netstring of netstrings.

The kinds are `spawn`, `line`, `call`, `ret`, `join`, `joined`, `stop` and `err`. A `spawn` message carries the definitions as s text lines and the captures as given names with their modes. A `joined` message carries the `mut` captures back. A `call` message ends with its chain of zone ids (§10.8). Thread crossings pass the same messages as values, so handles go with them.

10.12 **Partitions.** A `thread` zone, an `rpc` zone at `core`, a `net` zone and an `rpc` zone at a host each start a **partition** with every zone nested in them, unless they are already inside one. A partition runs on its own thread or host with its own scheduler. The partition that holds main is the coordinator. It spawns partition roots by sending their image, joins them by request, and ends the program when it is quiet and every partition has reported quiet after taking everything sent to it. Between partitions travel, as netstrings: `deliver` (a zone message), `spawn` (an image with the root's incarnation number), `spawnreq`, `joinreq`, `joined` (the `mut` captures of a root that ended), `joindone`, `status`, `tick`, `exit` and `halt`.

A net connection opens with the line `s3 <version> <hash> <partition> <root>`, where the hash is FNV-1a 64 of the world's built spec in its JSON form. The host answers `ok`, or a line saying why it refuses: another version, a different world, or no such zone. A coupled world opens with `s3 <version> couple` and is not checked.

10.13 **Losing a partition.** When a partition's thread panics or its connection closes, its root is lost: the error is `zone lost: name: reason`, the root's `mut` captures read as `zone lost: x` until written, its `ref` captures are released, and calls waiting on it fail with `zone lost: name`.

10.14 **Words.**

| word | args | effect |
|---|---|---|
| `spawn` | name | spawn the zone now |
| `join` | name | wait until the zone is quiet, end it, release its captures |
| `give` | name | in a gate body: move the name into the crossing |
| `zone` | | push the current zone's name |
| `host` | | push the current zone's `host:port`, or `local` |
| `inbox` | | push the number of waiting messages |

## 11. Text form

11.1 A world's text form is one text unit per zone. The unit for `main` is `world.s`, and the unit for a zone `z` is `world.z.s`. A unit holds:
- every shelf of the zone in generation order;
- every script plate of the zone as `{…} name$`;
- every sphere body of the zone as `{…} sphere:name$`, including the bodies of the gates on its walls and on its child zones' walls;
- the zone's main plate.

A graph table holding zones, spheres, links and params as JSON is kept in a file next to the units. A zone entry has `id`, `name`, `kind`, `min`, `max`, `params` and `captures`. A gate is a sphere entry with kind `gate` and the fields `zone` and `wall`. A world with no zones has one unit, which is the s 1.0 case.

11.2 These are added to s 1.0 lexing:
- `⟨word⟩` is a word that is never read as a suffix form.
- `:name` locals (§9).
- The geometry words (§8) and the zone words (§10.14), which act only when a graph runtime is present.

11.3 **Import.** An s file becomes a world:
- each line becomes a column on a `main` plate, in reading order;
- brace literals become box bricks with their inside laid out recursively;
- `{…} 'word' ><` lines at the top of the file move to a shelf, one column each, in order;
- `{…} name$` lines become script plates;
- other lines stay where they are.

11.4 **Export** of an imported world gives back its text, up to whitespace inside lines and the shelf move in §11.3.

## 12. Studio

The kit is fixed before building.

12.1 **Camera.** Holding the right mouse button flies: WASD plus the mouse, R and F for up and down, Q and E rolling as in Space Engineers, Shift fast. Releasing it returns to editing, where E opens panels. Tab toggles orbit around the selection. The full keymap is `BEVY.md` §4.7–4.8.

12.2 **Placing bricks.**
- Palette tabs: commands by group, library words by file, geometry words, and words already defined in the world.
- Clicking places a brick on the face under the cursor, and a ghost preview shows where it will go. A short right click removes a brick.
- Typing on a selected brick sets its word, with autocomplete. Enter places a new brick above and moves the cursor up, so a line is typed upward exactly as it is read.
- Ctrl+Enter starts a new column on the next cell along the plate's arrow.

12.3 **Line editing.** Double-clicking a column opens it as one line of s text in a bar below the view. Editing the text relays out the column.

12.4 **Selection.**
- Box select and multi-select.
- G moves, R rotates a selection about +y (columns stay vertical).
- Ctrl+D duplicates.
- Ctrl+C and Ctrl+V copy and paste, as s text and as world JSON.
- Delete removes.

12.5 **History and files.**
- Undo and redo cover every edit.
- The world autosaves in the browser.
- Export to `.s` plus graph JSON, to world JSON, to `.rs` (§13), and to a standalone page.
- Import from `.s` (§11.3) and from world JSON.

12.6 **Boxes.** Space opens a box in place, scaling the camera into it. Esc leaves.

12.7 **Spheres.** Spheres are dragged in from tiles, and the wheel pushes a held sphere nearer or further. E opens the panel for the hovered or nearest sphere. The panel has four tabs:
- Relations: links, with their mode, port, live state and a remove button.
- Body: the inside plate, editable in 3D or as text.
- Sphere: name, kind, position, aim, reach, params.
- Trace.

12.8 **Seeing a run.**
- The **stack** is drawn beside the executing column as translucent value cubes, bottom to top, each labelled. Frames parked by `(` show as a ledge holding their cubes.
- `ow` is a lamp on the plate's arrow.
- Pulses travel along links as particles. Live ray links run to the hit point; dormant ones are faint dashed lines.
- The brick that raised an error turns red and shows the message.

12.9 **Run controls.** Run, pause, step one token, step one column, step one pulse event, speed, and reset. A schedule switch chooses free or lockstep (§10.10); in lockstep, step one round is added. Breakpoints sit on bricks, on spheres and on gates.

12.10 **Finding things.**
- Search by word, which flies to each hit in turn.
- Hovering a word shows its resolution line (§4.3).
- Layer toggles hide shelves, boxes' insides and links.
- A minimap shows the top view.

12.11 **Zones.**
- Z then a drag draws a zone's box on a plate or in the air. Handles on the faces resize it. Contents belong to it by position, so resizing changes membership, and the studio shows the change before it is applied.
- Walls are translucent. The edges show the kind: `async` thin dashed, `thread` solid, `net` solid with the host on the top edge, `rpc` doubled. The top edge also shows the name and status (spawned, running, waiting, quiet, joined, lost), the event count, the inbox depth, and for `net` and `rpc` the round-trip time.
- **Captures.** A name brick, or a word picked from any column, dragged onto a wall becomes a capture brick set into the wall. Its mode is drawn as a shape: two offset tiles for `copy`, an arrow through the wall for `move`, an open ring for `ref`, a closed ring for `mut`. Clicking cycles the mode. A capture that cannot travel to the zone's kind turns red and shows the reason.
- **Gates.** The gate tile drops a gate onto the nearest wall, snapped to the wall's grid.
- **While running.** A moved name outside its zone shows as a hollow brick with a thin line to the wall that holds it. A lent name shows with a dark rim. A use that raises `moved:` or `lent:` turns red, with the line to the zone. Particles pause inside a gate while their message waits, and given names travel as small cubes beside the particle.
- Before a run, literal reads of a `move` capture in the parent's plates get a warning rim.
- **Zone panel.** E on a wall opens it, with four tabs:
  - Zone: name, kind, host, merge, budget, stack size, export, peer.
  - Captures: the list, with modes and what each name holds now.
  - Trace: this zone's trace.
  - Wire: the last 200 messages, decoded.
- Changing a zone's kind keeps everything in place.
- A `net` or host `rpc` zone whose host cannot be reached can be run as a thread zone from the panel. Its trace is then marked loopback.
- Alt+click on a wall dims everything outside the zone; Alt+click outside restores the view.

12.12 **Live edits.** An edit while a world runs is built and handed to the run. Each zone takes the new build at its next event boundary: its graph is replaced, keeping each sphere's ratchet count, shown value and error, and the runtime pose of a sphere that code moved and the edit did not; changed sphere bodies and scripts are redefined. A running event finishes with the code it began with. An edit that adds, removes or changes a zone, or that does not build, ends the run and says why.

12.13 **Look.** Light coffee with milk, no rounded corners, box shadows on panels. Brick faces show their word as crisp text on the face, with no background plate.

## 13. Compilation and running

13.1 **Browser.** `s3-core` compiled to wasm runs the program in a worker; the studio is the same Bevy application built for the web (`BEVY.md` §6).

13.2 **Rust.** The pipeline is world → text form (§11.1) → `s3-compile` → one `.rs` file containing:
- `s3-core` inlined: the machine, the graph runtime (§5–8, the §6.4 intersection, the §7 pulse loop, the clock, ratchet counts, activations) and the scheduler;
- the segments of every plate and box (`BEVY.md` §2.3);
- the graph table as statics.

`main` runs the shelves, then the main plate, then any `src` sphere that has `run = true`.

13.3 Natives (s 1.0 §14) carry over: a Rust file given with `-n` adds words.

13.4 Moving spheres from code works the same in both runtimes, because positions are part of the runtime state and §6.4 is exact.

13.5 **Zones in Rust.**
- The `.rs` file holds the segments of every zone, namespaced by zone id. Each zone runs in its own machine.
- Each thread has one scheduler that runs its zones round-robin with an operation budget (`BEVY.md` §2.8). An `async` zone is a zone on its parent's scheduler. A blocking word suspends the zone's machine at any depth, since the machine keeps its own stack.
- Each partition (§10.12) gets a scheduler on a new `std::thread` with a 64 MiB stack. Crossings go over `std::sync::mpsc` channels as values.
- A `net` zone runs on another machine that has started `s3 serve host:port world.json`, or the compiled program as `prog --serve host:port`. Both hold every zone's segments, so only the spawn image, the captures and the lines travel.
- `--serve` also runs a world that exports rpc gates and takes calls from coupled worlds (§10.8).
- `--lockstep` selects the §10.10 schedule on one thread. `--one-thread` runs every zone on main's thread in free mode. `--net-here` runs `net` zones and host `rpc` zones as threads of this process.
- The runtime uses `std` only.

13.6 **Zones in the browser.**
- A `thread` zone is its own Web Worker.
- `async` zones run on their parent's scheduler in the parent's worker.
- `net` zones and host `rpc` zones connect over WebSocket to `s3 serve --ws addr` or `prog --serve --ws addr`.
- `tcp` still fails in the browser, as in s 1.0.

## 14. Conformance

Every case runs in the browser runtime and in compiled Rust, under lockstep. Both must produce identical output and identical pulse traces. `net` cases run against a server on the loopback address that the harness starts.

| test | checks |
|---|---|
| `s10-corpus` | every s 1.0 test case, imported, run and exported, gives the same output |
| `column-order` | bottom-to-top tokens, row and arrow reading order |
| `caps` | each cap and its `?`/`!` collars give the matching s form |
| `angle-word` | `⟨x.⟩` defined and used as a word |
| `box-depth` | sockets at depth 1 interpolate, deeper text stays raw |
| `shelf-height` | a lower shelf's body cannot see a higher shelf's word |
| `shelf-replace` | replacing a definition brick keeps its generation |
| `locals` | `:x` in a definition does not touch the caller's `:x` or `x` |
| `locals-tail` | a tail call replaces the activation |
| `strike-sticky` | carried-over sphere graph rules 2.3, 2.4, 3.1 |
| `ray-dormant` | carried-over rule 3.2 |
| `ray-exact` | the §6.4 formula on tangent, inside and edge-of-reach cases |
| `pulse-order` | lowest link id first, depth first |
| `ratchet` | n arrivals on `loop`, then one on `exit`, then reset |
| `move-reroute` | `move` inside a body changes which link is live on the next emission |
| `budget` | the step budget ends a cycle without error |
| `ray-chain-space` | a two-sphere ray cycle runs 1 000 000 hops with the pulse stack never above one event |
| `round-trip` | import then export of each example gives back its text |
| `zone-copy` | the zone's writes to a copied name do not reach the parent |
| `zone-move` | reading a moved name in the parent is `moved: x`; writing it sets it fresh |
| `zone-ref` | a parent write while the zone is live is `lent: x`, a zone write is `borrowed: x`, and both clear at join |
| `zone-mut` | after join the parent holds the zone's final value |
| `zone-reborrow` | the child capture modes allowed from `ref` and `mut`, and the ones refused |
| `zone-give` | a given name leaves the sender and arrives set in the receiver |
| `zone-handle` | a handle moves to a `thread` zone; to a `net` zone it is refused |
| `zone-wall` | rays see only their own zone's spheres and gates; `move` out of a zone is an error |
| `zone-retag` | one world run as `async`, `thread` and loopback `net` gives identical per-zone traces |
| `zone-kahn` | a two-zone cycle with one sender each gives the lockstep traces in free runs |
| `zone-round` | `round` merge takes senders in id order |
| `rpc-return` | lines arriving back at the gate during a call are joined in order; an empty return emits nothing |
| `rpc-call-word` | `gate.` in a body replaces the line with the return |
| `rpc-reentry` | a call back into a zone with a pending call is `rpc reentry: name` |
| `rpc-couple` | two separately compiled worlds couple by gate name |
| `net-hash` | a mismatched spec hash refuses the zone |
| `live-edit` | a changed body runs from the next event; a changed zone tree ends the run |
| `zone-lost` | killing a `net` zone's process makes its `mut` names `zone lost: x` |
| `wire` | every message kind encodes and decodes identically in Rust and JS |

## 15. Limits

- One brick per cell. A column is one line; long lines are tall columns.
- Sphere positions are continuous and bricks are on the grid. A sphere's inside plate has its own grid.
- The studio runs every zone on the session's thread, so stepping, the stack view and live edits reach all of them. `s3 run` and compiled worlds give each partition its own thread or host.
- A `thread` zone inside another thread zone shares its partition's thread.
- From inside a partition, `join` reaches only zones of the same partition; `join` of another partition's zone is the error `join: name runs in another partition`. The coordinator joins any zone.
- Traces and shown values from other partitions arrive with their reports, when the partition settles, so free-mode traces are complete only per zone.
- Calls only go from a zone into a child zone, so a call chain cannot revisit a zone; the reentry check (§10.8) guards it anyway.
- Within a zone, pulses are sequential. s threads (`t`, `***`) still run inside bodies as in s 1.0 and do not carry pulses; a `thread` zone is the s³ form of a worker and does carry them.
- Only global names are captured. `:` locals stay in their activation.
- Handles do not leave their host. Definitions travel as text.
- Outside lockstep, `arrival` merge with several senders, clocks, and outside resources make the order of a zone's messages depend on timing.
- A `round` zone waits forever for a live sender that never sends.
- Rays cross walls only into gates.
- A lost zone loses its `mut` captures.
- Coupled worlds agree only on gate names and the wire format. Values are text on both sides, so nothing checks their shape across the coupling.
- Ray dispatch depends on positions. A program whose spheres move gives the same result in both runtimes only if the order of moves is the same, which §7.2 fixes.
- `⟨word⟩` and `:` locals extend s 1.0 text. Text that uses them does not run on the plain s 1.0 binary.
