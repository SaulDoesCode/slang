# The editor

`s3-studio --files <folder>` serves the editor at `http://127.0.0.1:7333/`: the .s files of that folder as text, in tabs, with the std files beside them read-only. The room (the same files as pages in space) is at `/room.html`, the worlds studio of s³ at `/studio.html`. F1 in the editor lists every key.

- **Files.** The folder as a tree with the std files under `std/`; Ctrl+P filters, a new `name.s` and Enter makes it. Each file is a tab (Alt+W closes, Alt+PageUp/PageDown moves); open tabs, carets, arguments and stdin are kept per file in the browser. Ctrl+S saves. A file changed on disk comes back into a tab with nothing unsaved.
- **Words.** Each word is coloured by the class the machine reads it as (the server's `lex`): numbers, commands, std words, lookups, sets, definitions, calls, loaded files. Braces are zones and bead lines are lanes, as in BEADS.md §4. Line numbers stand at the right edge.
- **Running.** Ctrl+Enter runs the file on the machine with a trace: what each line printed stands at the left of that line, a failed check (`fail got … want …`) in red, an error at its word and beside its line. Ctrl+Shift+Enter compiles to Rust, builds and runs the binary; with **fold** on (the default), what the program does before it first reaches outside the machine is done at build time (see the README), and the output's first line says how the program was built and how far it folded. A build is the `s3 build` command beside the server, run on the editor's text in the file's folder; a binary is kept per text in `.s3-build/`. Both run in the file's folder with the arguments and stdin of the input tab (`args`, `argc`, the lines `<_` reads). After a run each fork says how it ran and each join shows its lanes. Ctrl+R shows the Rust. Ctrl+` goes to the console, which runs s with the paragraphs of the file that define something or load a file.
- **Moving.** F12 or Ctrl+click goes to the definition of the word at the caret (`{…} key ><`, `{…} name$`): in the file, in the files it loads with `name.s.` and the files those load, or in std. Alt+← comes back. Shift+F12 marks the uses and lists them; Ctrl+Shift+O lists the file's definitions; Ctrl+G goes to a line.
- **Writing.** Completion opens after two letters or with Ctrl+Space: the file's definitions, the commands with their signatures, std words with their files, the names in the file. Resting the pointer on a word shows its signature and doc, the sigil it carries, and where it is defined. F2 opens the palette: every command by group and every std word by file, clicked in at the caret, and the sigils, clicked onto the word before it.
- **Finding.** Ctrl+F finds in the file (case, regular expressions), F3 and Shift+F3 step, Ctrl+H replaces one or all as one undo; Ctrl+Shift+F finds in every file of the folder.
- Ctrl+= and Ctrl+− set the text size, Ctrl+0 resets it.

Requests the editor adds: `srun {…, args?, stdin?}`, `build {…, args?, stdin?, fold?}` → `{…, how}`, `rustsrc {path, text, fold?}` → `{rs, note}`, `lib {path}` → a std file's text (with no path: `files`).

# The room

`/room.html` is the same folder of .s files, one at a time, as pages in space.

A file is plain s and stays plain s. Every paragraph (lines between blank lines) is a page: right-aligned curve text, colored by what the machine reads each word as, every level of braces underlined once more. Pages face the view and stay level to the screen. Where a page stands is kept beside the file in `.<name>.s.room`, one line per page moved by hand: `x y z {first line} page.`; pages never moved are spread out in file order, definitions a step back.

F1 in the room lists every key. Moving: W A S D, Space and C up and down, Q and E roll, Shift fast, the wheel sets the speed; Tab captures or frees the mouse. Editing: double click a page or aim and press Enter; the page opens in the box at the bottom at that character, and the caret and selection show on the page. Esc ends the edit; an emptied page goes, a blank line splits one. Saving is automatic and Ctrl+S saves at once; Ctrl+Z outside the box undoes whole edits. F5 runs the file: what each word prints stands beside its line, an error marks its word. L runs again after every edit. G flies to the definition of the word aimed at, B back; a thread joins a word to its definition, or a definition to its uses. The box, with no page in it, runs s with the file's definitions; `/text` searches. Ctrl+B builds the file to Rust and a binary beside it.

Requests (POST /api, JSON): `ls`, `read {path}`, `write {path, text}`, `srun {text, path?, prelude?, trace?, ms?}` → `{out, tags: [{at, text}], error: {at, msg}, forks: [{at, how}], ops, ms}` with `at` a character offset in the text, `lexdeep {text}`, `roomget {path}`, `roomput {path, text}`, `rust {path, text, build?}`, `build {path, text, ms?}` → the program compiled to Rust, built once per text into `.s3-build/` in the folder, run in the file's folder: `{out, stderr, error: {msg}, exit, forks: [{at, how}], ms, built, build_ms}`.

`/s/` leads to the editor at `/`.

On this machine, one fork over ten million numbers (`⟪xs…⟫ 2 \* =~ 1 \+ =~ 3 \/ =~ ⨝`) took 145 ms as a vector program and 8.9 s one lane at a time; with `ys into` building the ten-million-item list after it, 330 ms on the machine and 304 ms compiled (`core/tests/bench_lanes.rs`).

# s³ studio and server

The studio is a Three.js page served by `s3-studio`, a Salvo server. The page holds a mirror of the world for drawing; every edit, query, run and build goes to the server, where one core thread owns each world, its undo history and its running session.

## 1. Running

```
cargo build --release
./target/release/s3-studio [--port 7333] [--host 127.0.0.1] [--worlds worlds] [--studio studio] [--levels levels]
```

Open `http://127.0.0.1:7333`. Worlds are saved as `worlds/<name>.json` after every edit. The server binds to 127.0.0.1 by default: worlds run with the shell (`<`) and file commands of s.

## 2. API

`POST /api` with a JSON object `{cmd, name, …}`; the answer is JSON. `name` is the world: letters, digits, `_` and `-`. An answer with `error` changed nothing.

| cmd | arguments | answer |
|---|---|---|
| `list` | | `worlds`, `levels` (name, title, order) |
| `open` | `level?`, `reset?` | `world` (the world JSON), `undo`, `redo`. With `level`, the level is copied into the world first unless the world exists |
| `edit` | `edit` | `put` (records with `t`), `del` (ids), `result`, `undo`, `redo`, and while running `live` or `stopped` |
| `undo`, `redo` | | as `edit` |
| `palette` | | command groups, library words by file, graph words, caps |
| `lex` | `text` | `tokens` (`from`, `to`, `kind`, `w`), `error` |
| `resolve` | `brick` | what the brick's word stands for: `kind`, `doc`, `sig`, `column` of a definition, … |
| `complete` | `prefix`, `brick?` | `items` (`w`, `kind`, `doc`) |
| `typed` | `word` | what typing the word would make |
| `text` | | `zones` (name, s text) or `problems` |
| `world` | | `text`: the world JSON, pretty |
| `import` | `text` | a new world from s text |
| `run` | `lockstep?`, `breaks?` | a report; the session waits for `poll` or `step` |
| `poll` | `ops?`, `ticks?`, `watch?` | runs up to `ops` (default 200 000) and reports |
| `step` | `kind` (`token`, `column`, `event`, `all`) | runs that much and reports |
| `break` | `brick`, `on` | breakpoint on a brick |
| `fire` | `sphere` | queues a pulse from a sphere |
| `keep` | | runtime sphere positions and aims become an edit |
| `stop` | | ends the session |
| `compile` | | `rs`: the world as one Rust program |
| `build` | | `job`: compiles with rustc and runs, in the background |
| `job` | `job` | `done`, `ok`, `log`, `out` |
| `worldtext` | | `text`: the whole world as s (WORLDS.md §9) |
| `fromtext` | `text` | the world rebuilt from its text as one undo step: changes, `world`, `live` or `stopped` |
| `check` | | a level's goal: `ok`, `why`, `stars`, `bricks`, `events`, `output`, `shown` |

### Edits

| op | fields |
|---|---|
| `place` | `space`, `cell`, `word`, `note?`: types a word into a cell (replacing a brick there). `{…}` makes a box and lays its text out inside |
| `set` | `id`, `word?`, `cap?`, `cond?`, `kind?`, `note?` |
| `column` | `space`, `cell`, `line`: the column holding the cell becomes the words of the line |
| `paste` | `space`, `cell`, `text`: one column per line along the plate arrow |
| `settext` | `space` or `plate`, `text`: a plate's bricks become the text |
| `remove` | `ids`: bricks (with their insides), plates (with their bricks), spheres (with links and body), links, zones, datums |
| `move`, `duplicate` | `ids`, `delta` |
| `rotate` | `ids`, `pivot`, `turns?`: quarter turns about +y through the pivot cell, counterclockwise seen from above; bricks, plates with their bricks, spheres with their aims, datums (each turn adds 90 to `ry`) |
| `plate` | `space`, `kind`, `name`, `at`, `w`, `d`, `arrow` |
| `sphere` | `kind`, `name?`, `pos` or `cell`, `aim`, `code?`, `params?`, `strike?`, `reach?`, `space?`: with `cell` it is a device on the grid; kinds `src fn ratchet out gate mast port` |
| `setdown`, `lift` | `ids`, `cell?`: a sphere goes onto the grid as a device (remembering where it floated), or back to the sphere layer |
| `assemble` | `ids`, `name`: the spheres become the inside of an assembly placed where they were; links crossing the edge become ports and pinned links |
| `instance` | `assembly` (name or space id), `name?`, `pos` or `cell`, `space?`: another placement of an assembly |
| `link` | `from`, `to`, `mode?`, `port?`, `from_pin?`, `to_pin?`, `route?` (cells a wire runs over) |
| `zone` | `name`, `kind`, `min`, `max`, `captures?`, `params?` |
| `datum` | `cell`, `name?`, `value?`, `scale?`, `rot?`, `space?`: a datum cube; names default to `c1`, `c2`, … |
| `tile` | `id`, `face` (`+x -x +y -y +z -z` or 0…5), `u`, `v`: toggles that tile's bit; answers `bit` and `value`; logs `m c7;` or `m c7:` |
| `value` | `id`, `value`: sets all 24 bits; logs `v c7#` |
| `transform` | `id`, `scale?`, `rot?`, `parts?` (`{"sx": 2.5, "ry": 45}`): sets transform parts; angles wrap to (−180, 180]; each change logs `x c7/part#` |
| `clearlog` | `id`: folds a cube's log into the shortest program that rebuilds it |
| `put` | `t` and a full record: replaces it |
| `meta` | `name?`, `params?`, `goal?` |
| `batch` | `edits` |

After every edit the server refreshes strikes: an aim that now strikes a sphere makes a link, in the same undo step.

### Reports

`out` (output since the last report), `trace` (pulse events: sphere `s`, `in`, `out`, `port`, `via` link, `err`; zone spawns and joins; budget halts), `errors` (`msg`, `brick`, `sphere`, `zone`), `zones` (per zone: `phase`, `events`, `queued`, `inbox`, the `brick` being run, the `line`, `frames`, `ow`, `depth`, and the `graph`: sphere positions, aims, hits, notches, shown values, links with `live`; and `datums`: each cube's `id`, `value` and present `parts` as the zone's names hold them), `idle`, `halted`, `exit`, `stop`.

## 3. Studio

- **Menu and windows.** ≡ opens the menu: new world, import and export, the tutorial, the theme (coffee with milk, monokai pink, zen white and black), layers, resetting the window layout, keys. Every panel is a window: drag its strip to move it, double-click the strip or ▾ to fold it; windows snap to the screen edges and to each other, and a window docked at the bottom flips so its strip stays at the edge. The bar moves by its grip, top or bottom. The output sets its height by its free edge. Layout and theme are kept in the browser.
- **Console.** Backtick drops the output over the scene and focuses its input line. Enter runs s in the running session's main zone, or in a machine of the console's own between runs; ↑ ↓ walk the history.
- **Tutorial.** Chapters are the levels in order. A chapter opens as a world of its own with a guide window that follows its steps (each waits for an edit, a run or a solved check), and moves on to the next chapter.
- **Scopes and logic.** The graph tab paints scopes: pick a dialect, click two corners. A logic scope runs by ticks (the clock switch): switches are sources with an `on` param (the inspector or the hand in walk mode flips them), gates are function devices whose body is `and`, `or`, `not`, `xor`, `nand`, `nor`, `xnor` or any s, and an out device is a lamp that lights on 1. Gates on a scope's wall are its ports; their bodies convert in the scope's words (`bit`).
- **Walking.** F switches between flying and walking. Walking is first person in the grid: W A S D, the mouse looks (the pointer is held), Space jumps and vaults a ledge one cell high, Shift sprints, Ctrl crouches under a one-cell gap, C slides out of a sprint. Bricks, devices and plates are solid; spheres are not. The bar on the right holds the equippables (brick with its word, wire, the device kinds, datum cube, gimbal, assemblies, inspect, remove); the wheel, the middle button and 1–9 pick, left click uses, right click turns a device a quarter or removes a brick. The hand, or the datum cube in hand, toggles the aimed tile. With the gimbal in hand the wheel scales the aimed cube by eighths and Shift+wheel turns it by 15°; X, Y or Z keeps the gimbal to one axis, the same key again frees it. Scaled cubes are solid where they stand, so they build walls and floors. E carries the aimed sphere, device or cube and sets it down where the crosshair is, Q drops it here, R rides the aimed link and on along the links after it. Enter sets the brick's word. Esc frees the pointer and opens the menu; a click on the view walks on. The menu packs the bar.
- **Camera.** Hold the right mouse button to fly: W A S D, R F up and down, Q E roll, Shift fast, wheel sets speed. A short right click removes the brick under the cursor. Middle drag pans, the wheel moves forward. Home frames the space. Tab switches to orbiting the selection: the right mouse turns around it and the wheel zooms; Tab again flies. The minimap shows the space from above with the camera as an arrowhead; a click there moves the camera to look at that point.
- **Typing.** Click a plate cell and type; Enter places the word and moves up the column, so a line is typed in the order it is read. Ctrl+Enter starts the next column along the arrow. Completions come from the server; Tab or ↑ ↓ take one. A trailing sigil becomes the brick's cap, a collar `?` or `!` its collar.
- **Bricks.** Click selects, Shift+click adds, Alt+click stacks a new brick on that face. A drag from empty space, or Ctrl+drag anywhere, selects the bricks inside the rectangle. Enter edits, Delete removes, Alt+arrows and Alt+PgUp/PgDn move, G grabs the selection so it follows the cursor on its level (a click places it, PgUp/PgDn lift it, Esc cancels), R and Shift+R turn it a quarter about its middle, Ctrl+D duplicates, Ctrl+C copies as s text, Ctrl+V pastes text as columns. Double-click opens the column as one line, colored by the server's lexer. Resting the cursor on a word shows what it resolves to.
- **Find.** Ctrl+F or / goes to the find box beside the output: matching bricks turn yellow and matching sphere names gold; Enter flies to the next match, entering its room, and Shift+Enter goes back.
- **View.** Layers for shelves, links, zones, sphere names and the minimap; the choice is kept in the browser.
- **Rooms.** Space opens a box or a sphere body as its own space; Esc leaves. The crumbs show where you are.
- **Devices, wires, masts, assemblies.** In the graph tab, "place on the grid" makes new spheres devices standing in cells, on plates or on the ground. The inspector sets a sphere down onto the grid or lifts a device back up. A mast stands on a cell and reaches up to the sphere layer. The wire tool links one sphere or device to another; wires run over the ground as tubes, beams (ray links) as straight rays. Shift+click gathers spheres; the inspector assembles them under a name; the assembly then appears in the graph tab to place again, and Space opens its inside. Links to and from a placement name its ports with @.
- **Spheres.** The graph tab places spheres. Drag moves; the wheel while dragging pushes nearer or further; Shift on drop aims along the move. Alt+drag onto a sphere aims at it, and the strike links them. E opens the inspector: name, kind, strike, reach, params, the body as text, relations with mode and port, remove.
- **Datum cubes.** The graph tab's datum tool sets a cube on the ground, on a plate or against a face. A cube shows its 24 bits as tiles, white for 1, black for 0, over a body of its color; beyond 12 cells it is one flat block of its color, so a field of cubes seen from afar is an image, and the minimap draws them as pixels. Click selects a cube; a click on a selected cube toggles the tile under the cursor (Alt toggles at once, Shift only selects). Right click removes. T takes the gimbal: knobs scale along the cube's own axes, squares scale two axes, the center scales all three, rings turn it about x, y and z. Ctrl snaps to sixteenths of a cell and 15°; typing a number while dragging sets the amount exactly; X, Y or Z switches the drag to that axis; Esc cancels and Enter or the release sets it. A cube stands on its cell's floor: scale and rotation pivot there. The inspector shows the tiles by face, the schema parts as fields, and the mutation log with Copy and Fold. While a program runs, the cubes show what code left in their names.
- **Plates and zones.** P and Z make one in front of you; Shift+click a plate or click a zone wall to edit it. Zone captures are a list of names with modes.
- **Running.** Run (F5), Pause (F6), Token (F10), Column (Shift+F10), Event (F9), Stop. The running brick turns orange, the line is drawn beside it with the parked frames and ow, pulses travel along links, errors turn their brick red and click through to it. Breakpoints: F8. Lockstep and clock are switches; trace lists every pulse event.
- **Files.** Text shows the whole world as s text, editable and applied as one undo step, and the program per zone. Build compiles to Rust with rustc and runs it. File imports s programs and exports world JSON and `.rs`.

An edit while a session is running goes to the session: each zone takes it at its next event (SPEC §12.12). The answer to `edit`, `undo` and `redo` then carries `live: true`, or `stopped` with the reason when the edit changed a zone or does not build, in which case the session has ended.

Bricks show their word whole on each open face, fitted to it; a box shows the code inside it as s, on its sides and top, so nothing hides behind braces. Pointing at a brick shows its whole line, the brick bracketed. The walker stands two and a half cells tall: code is bench height, devices are parts on a board.

Tracks: the graph tab has a track tool (click the start cell, then the end; it runs along x, then z) and one tile per machine; the walk bar packs `track` and each machine. Right click or Delete removes a track under the cursor. Items show as cubes in their color, bars in their channel, text as text, easing between ticks.

## 4. Levels and chapters

A level is a world file in `levels/` with a `goal`:

```json
{"order": 1, "title": "…", "text": "…", "output": "hello", "shown": {"sphere": "32"}, "par": {"bricks": 2, "events": 13}, "ticks": 0}
```

`steps` (optional) is the guide: a list of `{"on": event, "text": …}` where the event is an edit op name (`place`, `sphere`, `link`, `settext`, `put`, `setdown`, `assemble`, …), `run` or `solved`. `output` is what the program must print (trimmed); `shown` maps out spheres to what they must show; `datums` maps cubes to the value they must hold when the program is done, or to an object of `value` and parts (`{"c1": {"value": 255, "sy": 2}}`). The guide also knows `tile` and `far` (a cube toggled by the player seen from beyond 12 cells). Solving gives one star; meeting the par counts shares out the other two. Opening a level copies it into `worlds/level-<name>.json`, so progress is kept; stars are kept in the browser.

The ladder: a column, numbers, caps, shelves, spheres, ratchets, zones with captures, calls through a gate, a clock, giving a name away, a datum cube.
