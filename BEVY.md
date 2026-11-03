# s³ on Bevy

An implementation of s³ (`SPEC.md`) as a Rust workspace. The core is a library with no Bevy dependency. The studio is a Bevy application built on it. The same core runs in the studio, on the command line, in compiled programs, and in the browser as wasm.

## 0. Versions

0.1 Pinned exactly in `Cargo.toml`:

| item | version |
|---|---|
| Rust | 1.95 or later |
| bevy | `=0.19.1` |
| bevy_egui | `=0.42.0` |
| fdsm, fdsm-ttf-parser, ttf-parser | `0.8`, `0.2`, `0.25` (studio only) |
| rfd | `0.17` (native studio only) |

0.2 Bevy 0.20.0 with bevy_egui 0.43.0 requires Rust 1.97.1. Moving to it is a separate change. That change consists of the version bump, the fixes Bevy's migration guide calls for, and a rerun of the §9 performance suite. Nothing in the core changes with a Bevy version.

0.3 Bevy features are listed explicitly, with default features off. Audio and gamepad support are not included. No system audio or udev library is needed. The native studio starts from this list:

```toml
bevy = { version = "=0.19.1", default-features = false, features = [
  "default_app", "3d_bevy_render", "std", "bevy_winit", "multi_threaded",
  "x11", "wayland", "png", "tonemapping_luts", "bevy_gizmos",
] }
bevy_egui = { version = "=0.42.0", default-features = false, features = [
  "render", "default_fonts", "manage_clipboard",
] }
```

The web build adds `webgl2`, or `webgpu`, and drops `x11` and `wayland`.

## 1. Workspace and the gap

1.1 Crates:

| crate | depends on | contains | never contains |
|---|---|---|---|
| `s3-core` | `std` only | value machine, world document, edits, queries, text form, build, graph runtime, zone scheduler, wire format | rendering, input, camera, colors, Bevy |
| `s3-host` | `s3-core`; `std::thread` and `std::net` natively; `web-sys` on wasm | the run host: transports and reports | Bevy |
| `s3-compile` | `s3-core` | world → one `.rs` file | |
| `s3-cli` | the three above | the `s3` command (§8) | |
| `s3-studio` | `s3-core`, `s3-host`, bevy, bevy_egui, fdsm, rfd | projection, input, tools, panels, run view | semantics |

1.2 `s3-core` and the native part of `s3-host` use only `std`. This is what lets `s3-compile` inline them into one self-contained `.rs` file that `rustc -O` builds with no Cargo, as s 1.0 does. JSON and the binary world format are written in the core.

1.3 **Semantics live in the core.** The studio never evaluates s, resolves a name, decides a strike, orders a pulse, computes reading order, or decides whether an edit is valid. It asks the core through the queries in §2.9 and through `World::check`.

1.4 **The document is the truth.**
- `s3_core::World` is held by the studio as a Bevy resource.
- Entities and meshes are a projection of it.
- Despawning every view entity and projecting again gives the same picture. A test checks this (§10.3).

1.5 **Every change is an `Edit`.**
- `World::apply(&mut self, Edit) -> Result<Applied, EditError>`, where `Applied { inverse: Vec<Edit>, changes: Vec<Change> }`.
- Undo applies the inverse.
- `World::check(&Edit)` answers whether an edit would apply, without applying it. Ghost previews use it.
- The core enforces every world invariant: one brick per cell, plates inside one zone, zones nesting without overlap, gate centers on walls, unique sphere names, at most one link per pair. The studio cannot produce a world that breaks one.

1.6 **The run belongs to the host.**
- During a run, the host holds the program and all runtime state.
- The studio receives reports (§3) and draws them as overlays. A run never mutates the studio's `World`.
- Spheres moved by code during a run are runtime state. The Keep command turns their current runtime positions into one edit.

1.7 **Ids.** Bricks, plates, spheres, links and zones have ids `Id<T> { index: u32, gen: u32 }`. An id is never reused with the same `gen`. Compiled code carries a source map from each operation to its `Site { brick: Id<Brick> }`, which errors, breakpoints and watches use.

1.8 The studio crate is a library with a thin `main`. A user crate can depend on `s3-studio` and add natives (s 1.0 §14) and Bevy plugins:

```rust
App::new().add_plugins(StudioPlugin::default().natives(ext::NATIVES)).run();
```

## 2. Core

2.1 **Values.**
- A value is immutable text with a cached number parse.
- Texts up to 22 bytes are stored inline. Longer texts are `Rc<str>`.
- Zones share nothing, so nothing in an interpreter is `Arc`. Values that cross go through the wire format (s³ §10.11).

2.2 **Evaluator.** An explicit-stack machine.
- Every unit of text (a plate, a box, a script, a definition body, text run by `~`) is compiled once into `Code { ops, sites }`. Code is cached by the hash of its text.
- A word that a definition may replace compiles to a lookup with an inline cache keyed by the definition generation, as s 1.0 compiled code does with `rt.unreplaced`. Definitions, generations, binding, dispatch re-classification and escapes keep s 1.0 semantics exactly.
- Machine state: the value line, the frame stack `Vec<Frame { code, pc, act }>`, the activations holding `:` locals, and the ow flag.
- A call pushes a frame, a tail call replaces it, and a return pops it. Evaluation never recurses in Rust. Recursion depth is limited by memory only.
- `Machine::run(&mut self, budget: u32) -> Status` runs at most `budget` operations. It returns `Ran` (budget spent), `Wait(Wait)` (a blocking word), `Done`, or `Error(Error, Site)`. All preemption is built on it: the studio's frame budget, single-threaded wasm, several zones sharing a thread, stepping, and lockstep.

2.3 **Compiled form.**
- `s3-compile` emits each code unit as Rust functions split at calls and blocking words: `fn seg_N(m: &mut Machine) -> Next`, where `Next` is `Go(seg)`, `Call(code, ret_seg)`, `Tail(code)`, `Ret` or `Wait(wait, resume_seg)`.
- The s 1.0 fused fast paths stay inside segments.
- Compiled frames and bytecode frames share one layout, so compiled code calls text run by `~` and the reverse.
- Control returns to the machine loop at calls, returns and waits, never between plain operations.

2.4 **World document.**
- Cells: a map from chunk key to a dense `[Option<Id<Brick>>; 4096]` for 16³ cells. A lookup is one hash and one index.
- Bricks, plates, spheres, links and zones live in slabs.
- Each plate has a version counter. Its reading order is computed when asked and cached until its version changes.
- Zones are few and kept in a flat list sorted by volume; the innermost zone of a point is a linear scan.

2.5 **Change.** `Change` is one of:
- `Brick(Added | Removed | Changed, id, cell)`;
- `Plate(id)`;
- `Order(plate)`;
- `Sphere(Added | Removed | Changed, id)`;
- `Link(Added | Removed, id)`;
- `Zone(Added | Removed | Changed, id)`;
- `Captures(zone)`;
- `Meta`.

The studio maps each to the view work in §4.2.

2.6 **Build.**
- `build(&World) -> Result<Program, Vec<Problem>>` produces the text units (s³ §11.1), their compiled code, the graph table and the source maps.
- The build is incremental. Code for a plate is reused while the plate's version is unchanged.
- A `Program` serializes to bytes, which is how it reaches a host in a worker.

2.7 **Graph runtime.**
- Positions and aims are `f64`. The strike decision is only the s³ §6.4 formula.
- **Accelerator.** Each zone keeps a uniform grid with cell size `params.grid` (default 4.0). A sphere is registered in every grid cell its bounding box touches. A ray's candidates are the spheres registered in the grid cells its segment `[0, reach]` passes through, found by a 3D DDA.
  - The accelerator only selects candidates. It never decides a hit. Registration rounds bounding boxes outward, so the candidate set always contains every sphere the formula could hit. With the decision left to the formula, results equal those of a brute-force test of every sphere.
- **Refresh.** A strike refresh (s³ §7.2) is skipped when the step changed no position, aim or reach. Otherwise it rechecks the moved sphere's own ray, plus the rays registered as passing through the grid cells of the sphere's old and new bounding boxes. Its cost is proportional to the grid cells crossed, not to the number of spheres.

2.8 **Scheduler.**
- One `Scheduler` per OS thread, or per worker on wasm, owns the zones placed on it.
- It runs them round-robin, each with `Machine::run(slice)`. The default slice is 4096 operations.
- An `async` zone is a zone on its parent's scheduler. A `thread` zone gets a new scheduler on a new thread natively, or in a new Web Worker on wasm. A worker loads the same wasm module and exchanges wire bytes by `postMessage`, so no shared memory is needed.
- Lockstep (s³ §10.10) is a scheduler mode in which every zone runs one event per round.

2.9 **Queries.** Pure functions on `&World`, or on `&World` plus the last report. Each has a cost bound, because the studio calls them while hovering and typing.

| query | returns | bound |
|---|---|---|
| `reading_order(plate)` | columns in order | cached; O(columns) after a change |
| `column_text(col)`, `plate_text(plate)` | s text | O(bricks) |
| `lex(text)` | token kinds and spans, for coloring | O(len) |
| `resolve(brick)` | builtin, definition (shelf brick and generation), script plate, sphere, local, native, or unknown | ≤ 50 µs |
| `complete(prefix, at)` | words visible at that brick, with kind and doc line | ≤ 1 ms for 10 000 visible words |
| `docs(word)` | the library doc line | O(1) |
| `ray(sphere)` | end point and hit, for display in edit mode | as §2.7 |
| `capture_problems(zone)` | captures that cannot travel to the zone's kind | O(captures) |
| `warnings()` | literal reads of moved names, unknown words, empty boxes run as code | incremental per plate |
| `check(edit)` | `Ok` or the reason | O(1) for single-cell edits |
| `import_text`, `export_text`, `to_json`, `from_json`, `to_bin`, `from_bin` | | linear |

## 3. Host and reports

3.1 The host runs on its own thread natively and in a dedicated Web Worker on wasm. The studio's main thread never runs s.

3.2 **Commands** (studio → host):
- `Load(Program)`, `Run`, `Pause`, `Stop`.
- `Step(Token | Column | Event | Round)`.
- `Break(Site, on)`, `Watch(WatchLevel)`.
- `Patch(Vec<Edit>)`.
- `Schedule(Free | Lockstep)`.
- `Speed(ops per frame, or Max)`.
- `Keep`.
- `TraceRange(zone, from, to)`.

3.3 **Reports** (host → studio). The host sends at most one report per 16 ms, and one after every step command. A report holds:
- per zone: status, event count, inbox depth, round-trip time;
- sphere positions and aims that changed since the last report;
- the live-link set as a diff;
- for each watched activation: the stack line, frames, ow and current site. The line shows its first and last 32 values, each cut to 256 bytes, plus the full count and length;
- program output appended since the last report;
- errors with their sites;
- trace counts per zone. Trace entries themselves stay in a ring buffer of 65 536 per zone in the host, and the studio reads ranges with `TraceRange` as the Trace tab scrolls.

3.4 **Watch levels.**
- `Off`: pulse level only. This is the cost of a headless run plus the reports.
- `Selected` (default): token level for activations whose code contains the selected column or body.
- `All`: token level everywhere, at a cost that §9 measures.

3.5 **Breakpoints.** Each `Code` has a flag that is set when one of its sites has a breakpoint, and the machine checks sites only in code with the flag. A run with no breakpoints pays nothing for them.

3.6 **Live patch.**
- An edit made while running is sent as `Patch`. It applies at the next event boundary of the zone it touches.
- Geometry takes effect at the next strike refresh.
- Code edits rebuild the affected code units. Activations already running finish on their old code, and new events use the new code. A replaced definition keeps its generation (s³ §4.5).
- The status bar shows that the run has been patched.

3.7 **Backpressure.**
- The host never waits for the studio.
- State in reports is latest-wins. Output beyond 1 MiB per report is cut, and the cut is shown.

## 4. Studio

4.1 **Plugins.** `StudioPlugin` is made of:

| plugin | owns |
|---|---|
| `DocPlugin` | the `World` resource, undo, autosave, files |
| `ProjectionPlugin` | entities and meshes from `Change` lists |
| `BrickRender` | chunk meshes and brick text |
| `GraphRender` | spheres, links, gates, billboards |
| `ZoneRender` | walls, edges, capture bricks, status text |
| `RunPlugin` | the host connection and run overlays |
| `CameraPlugin` | fly, edit, orbit and focus |
| `PickPlugin` | what is under the cursor |
| `ToolPlugin` | tools that turn input into edits |
| `UiPlugin` | egui panels and theme |

4.2 **Frame order.**
1. Input.
2. Picking.
3. Tools produce edits.
4. `World::apply`.
5. Changes are routed:
   - brick changes mark their chunk dirty, and their neighbor chunk too when the cell is on a chunk border;
   - sphere changes update the sphere entity;
   - link changes mark the link mesh dirty;
   - zone and capture changes update the zone entity;
   - order changes mark the plate's arrow overlay dirty.
6. Remeshing of dirty chunks.
7. Reports are applied to the overlays.
8. UI.
9. Render.

4.3 **Bricks.**
- Bricks are not entities. Each 16³ chunk has one opaque mesh and one text mesh.
- Faces between two adjacent bricks are culled. Vertex colors carry the kind color.
- Caps and collars are instanced meshes, one per shape (s³ §2.2).
- Selection, hover, breakpoints, errors and the running brick are drawn by an instanced overlay of outlines and tints. A state change never remeshes a chunk.
- Remeshing runs on Bevy's async compute pool natively. A chunk is remeshed at most once per frame.

4.4 **Text.**
- One MSDF glyph atlas, generated with fdsm from an embedded font, covers:
  - printable ASCII;
  - Latin-1 and Latin Extended-A, which include the Afrikaans letters;
  - the s³ marks, including `⟨ ⟩ ³ → …`.

  Glyphs outside it are added to the atlas on first use. The native build caches the atlas on disk.
- Text is drawn on the four side faces of every brick. A word is scaled to fit the face, never below a minimum size. A word longer than 10 characters is cut with `…` on the face and shown in full on hover.
- The shader takes the median of the three channels and anti-aliases with screen-space derivatives. Edges stay sharp from 0.3 to 200 units, and there is no blur at any distance. Below 6 pixels of glyph height, text fades out and is not drawn.
- No text has a background plate.
- Out-sphere billboards and labels use the same atlas. Billboards stay level to the screen, as in the mind palace.
- The renderer is behind a `GlyphRenderer` trait, so a curve renderer for close-up quality can replace MSDF later without touching anything else.

4.5 **Spheres, links, zones.**
- Spheres are entities with an instanced sphere mesh at two levels of detail, one material per kind.
- Links are not entities. One ribbon mesh of camera-facing quads holds all of them. It is rebuilt when a link changes or when a sphere's position or aim in a report changes. Sticky links are solid; ray links run to the hit point; dormant ray links are dashed in the shader.
- Pulses are instanced particles that move along links, timed from the reports.
- Zone walls are translucent quads, and their edges are ribbons styled by kind (s³ §12.11).

4.6 **Look.**
- Light coffee with milk, with no dark coffee tones.
- One directional light plus ambient light. Shadows are off by default and can be switched on. MSAA is 4×.
- The egui theme has corner rounding 0 everywhere, box shadows on windows and popups, and the same palette.
- UI scale and font size are settings.

4.7 **Camera and modes.** There are two modes, so that keys never need to mean two things at once.

| mode | entered by | keys |
|---|---|---|
| edit | default; cursor free | as in §4.8 |
| fly | holding the right mouse button; cursor captured | W A S D move, R F up and down, Q E roll as in Space Engineers, Shift fast, Ctrl slow, wheel sets speed |

- Releasing the right button returns to edit mode, at the place the flight ended. Approaching a sphere in fly mode, releasing, and pressing E opens its panel, the way the sphere graph used E.
- A right click shorter than 200 ms and under 4 px of movement is a click, which removes the brick under the cursor. A longer hold flies.
- Tab toggles orbit around the selection. Alt + left drag orbits once.
- Home frames the selection with a 250 ms move, and Ctrl+Home levels the horizon.

4.8 **Edit-mode keys.**

| key | action |
|---|---|
| left click | place the current palette item on the face under the cursor; the ghost previews it, using `World::check` |
| right click | remove |
| Shift + left drag | box select; Ctrl + click adds or removes one |
| Enter | edit the selected brick's word; while editing, commit and open a new brick above |
| Ctrl+Enter | commit and start a new column at the next cell along the plate's arrow |
| Tab (while editing) | accept the completion |
| Esc | cancel the edit, close the panel, leave a box, or clear the selection, in that order |
| double click on a column | open the line bar (§4.10) |
| G, R | move the selection; rotate it 90° about +y |
| Ctrl+D, Ctrl+C, Ctrl+V, Delete | duplicate, copy, paste, delete |
| Ctrl+Z, Ctrl+Shift+Z | undo, redo |
| Space | open the selected box; Esc leaves |
| E | panel for the sphere, gate or wall under the cursor or nearest the view center |
| Z | zone tool |
| Ctrl+F, N, Shift+N | search; next and previous hit |
| F5, F6, F10, Shift+F10, F9, Ctrl+F9 | run, pause, step token, step column, step event, step round |
| F8 | toggle a breakpoint on the hovered brick or sphere |
| Ctrl+S, Ctrl+O | save and open |

While a text field has focus, only Esc, Enter, Ctrl+Enter and Tab reach the studio. Everything else goes to the text.

4.9 **Typing.**
- The word editor is an egui field anchored to the brick's projected position.
- Completions come from `complete(prefix, brick)`. Each shows its kind color and doc line.
- Typed text with spaces becomes one brick per word, stacked upward, so pasting an s line builds a column.
- A word typed with a sigil at its end gets that cap, unless it is written `⟨…⟩`. A `{` opens a box and puts the editor inside it, and `}` closes it.
- Undo takes one step per commit. A drag is one step. History is bounded at 10 000 steps.

4.10 **Line bar.**
- The line bar is a bottom panel showing the column as one s line, colored with `lex` in the s editor's colors and right-aligned as in the s editor.
- Enter relays out the column as one edit, and Esc discards.
- Lexing errors are underlined in place, and the column is not changed while one remains.

4.11 **Panels.**
- **Top bar:** file, run controls, schedule switch, speed, watch level, status.
- **Palette (left):** tabs for commands by group, library words by file, geometry words, zone words, and words defined in this world. Hovering an item shows its doc line.
- **Inspector:** opened by E. It floats beside its target, clamped to the screen. Its tabs follow s³ §12.7 and §12.11.
- **Console (bottom):** program output and errors. Clicking an error flies to its site.
- **Minimap:** a second orthographic camera rendering at 256², updated at 10 Hz, with the view frustum drawn on it.

4.12 **Files.**
- Ctrl+S saves world JSON. Export menu:
  - `.s` units with graph JSON;
  - `.rs`;
  - a page, from the web build only.
- **Autosave** writes the binary format 2 s after the last edit and on exit, on a background thread from a snapshot. It keeps 10 rotating versions. If the autosave is newer than the file on start, the studio offers to recover it.
- **Native:** files go through `rfd` dialogs. Autosave goes to `$XDG_DATA_HOME/s3/autosave/`.
- **Web:** files are uploaded and downloaded. Autosave goes to IndexedDB, with every access wrapped so a blocked store only disables autosave.

4.13 **Build and run natively.**
- The native studio's Build button exports `.rs` and runs `rustc -O` in the background, showing progress and errors.
- It then starts the binary with its output in the console.
- If `rustc` is not on the path, the button says so and still exports the file.

4.14 **Natives in the studio.** Natives declared by the studio's `main` (§1.8) run in the studio. A world that names natives the studio does not have shows them as declared but unavailable. Running one is the error `native not in studio: name`.

## 5. Picking

5.1 The cursor ray is tested in the core:
- bricks by DDA through the cell map, which gives the exact cell and face;
- spheres and gates by the §6.4 formula;
- links by screen distance of 6 px or less;
- walls by plane test.

5.2 Priority: gate, sphere, brick, link, wall, plate.

5.3 Picking runs only when the cursor or camera has moved. Its cost is at most 0.1 ms.

5.4 When egui wants the pointer, world picking and world clicks are off. Bevy's picking backends are not used for the world.

## 6. Web build

6.1 **Target.**
- The web build targets `wasm32-unknown-unknown` with wasm-bindgen.
- It renders with WebGL2, or with WebGPU when the browser has it (a build feature).
- Custom shaders use no storage buffers, so they work on WebGL2.

6.2 **Workers.**
- The host runs in a Worker, a second instance of the same module started at `host_main`.
- Thread zones run in further Workers.
- Net zones use WebSocket.
- Nothing needs SharedArrayBuffer or cross-origin isolation.

6.3 **Size.** Release builds use `opt-level = "z"`, LTO, `wasm-opt -Oz`, and the explicit feature list.
- The gate is 15 MB uncompressed. At that size the studio can be published as a page, with the wasm as one file.
- §9 measures the size.

## 7. Compiled programs

7.1 `s3-compile` writes one `.rs` file holding:
- the inlined `s3-core` and native `s3-host`;
- the segments of every zone (§2.3);
- the graph table as statics.

`rustc -O prog.rs` builds it.

7.2 Flags:
- `--lockstep`;
- `--serve host:port` and `--serve --ws host:port`, which serve net zones and exported rpc gates;
- `--budget n`;
- `--trace file`.

## 8. Command line

| command | does |
|---|---|
| `s3 run world [--lockstep] [--budget n]` | run headless, output to stdout |
| `s3 build world -o prog.rs` | compile |
| `s3 serve addr [--ws]` | serve zones and exported gates |
| `s3 import file.s -o world.json`, `s3 export world -o dir` | text form |
| `s3 test` | the s 1.0 corpus and the s³ conformance table |
| `s3 bench` | the §9 suite |

`s3 run file.s` runs an s 1.0 program, so the CLI replaces the s binary.

## 9. Performance

9.1 Reference machine: 4 cores, 16 GB, an integrated GPU of the Intel Iris Xe class, 1920×1080. Reference scene:
- 100 000 bricks in 20 000 columns;
- 2 000 spheres and 10 000 links;
- 8 zones;
- 200 visible text faces per chunk at most.

9.2 Gates. `s3 bench` measures each one. A release fails if any target is missed or any figure regresses by more than 20% from the previous release.

| measure | target |
|---|---|
| frame time, reference scene, idle camera | ≤ 16.6 ms; main-thread CPU ≤ 8 ms |
| frame time, reference scene, run at Watch `Selected` | ≤ 16.6 ms |
| one-brick edit to visible | the next frame; chunk remesh ≤ 1 ms |
| load and first frame, reference scene | ≤ 1.5 s native |
| autosave of the reference scene | ≤ 20 ms of main-thread time |
| build, reference scene, from scratch / after one edit | ≤ 50 ms / ≤ 2 ms |
| bytecode machine, simple operations | ≥ 30 million per second |
| compiled loop, the s 1.0 benchmark | ≤ 20 ns per iteration (s 1.0: 12–13 ns) |
| pulse step with one moving sphere, 2 000 spheres | ≤ 2 µs |
| pulse step with no geometry change | ≤ 0.3 µs plus the body |
| crossing to an `async` zone / a `thread` zone | ≤ 0.5 µs / ≤ 5 µs one way |
| hover query (`resolve`) | ≤ 50 µs |
| web build size | ≤ 15 MB uncompressed |

9.3 Bricks and links are meshes, not entities, so the ECS holds a few thousand entities in the reference scene. Rendering work grows with chunks and links, and run work grows with dirty state.

## 10. Tests

10.1 **Core.**
- The s 1.0 corpus runs on the bytecode machine and as compiled programs. It is the first gate, ahead of all other work.
- The s³ conformance table runs, under lockstep.
- Property tests:
  - random edit sequences followed by undoing all of them give back the starting document;
  - import then export gives back the text;
  - the accelerator agrees with a brute-force ray test on 100 000 random cases, including tangent and edge-of-reach rays;
  - wire messages decode to what was encoded.

10.2 **wasm.** The core tests also run under wasm. Lockstep traces must be identical to the native ones.

10.3 **Studio logic.** These are headless Bevy apps with no renderer:
- Scripted input sequences must produce the expected edits. For example, Enter, `a`, Enter, `b`, Enter, `+`, Enter gives the column `a b +`.
- Projecting from scratch must equal incremental projection, compared by hashes of entities and meshes.

10.4 **Pictures.** Reference scenes are rendered by the web build in headless Chromium and compared with stored images within a tolerance. Native screenshots are taken with Bevy's screenshot API on a machine with a GPU.

10.5 **Performance.** `s3 bench` on the reference machine (§9).

## 11. Order of work

Each step ends in something usable:

1. **Bytecode machine.** The s 1.0 corpus and the machine figures in §9 pass. `s3 run` replaces the s binary.
2. **Core world.**
   - document, edits, queries, text form, build;
   - graph runtime with the accelerator;
   - every conformance case that does not involve zones.
3. **Studio shell.**
   - brick, text, sphere and link rendering;
   - camera, picking, placing, typing, undo, save, autosave.

   It is usable as an editor.
4. **Run view.** Host, reports, overlays, stepping, breakpoints and live patch.
5. **Zones.** Scheduler, threads, wire, net, rpc, and the zone tools and panel.
6. **Web build, compile and finish.** The web build, `.rs` export with Build and run, minimap, search and the page export.

## 12. Changes to SPEC.md

These changes are already made in `SPEC.md`.

12.1 s³ §13.1 and §13.6: the browser runs `s3-core` compiled to wasm. `srt.js`, `graph.js` and `srt-gen.js` are not part of s³. `srt.js` stays the runtime of the s 1.0 editor.

12.2 s³ §13.5: baton groups are replaced by one scheduler per thread with operation budgets (§2.8).

12.3 s³ §12.1 and §12.2: fly mode is entered by holding the right mouse button, a short right click removes, and E opens the panel in edit mode (§4.7).

12.4 s³ §7.6: calls inside bodies do not use the host stack in any position. Recursion is limited by memory.

## 13. Risks

| risk | decision |
|---|---|
| Bevy API changes every release | exact pins; the core has no Bevy; upgrades are separate changes with the §9 suite |
| bevy_egui follows Bevy with a delay | the pair is pinned together |
| the machine rewrite changes s 1.0 behavior | the corpus is the first gate |
| text quality | MSDF with derivative anti-aliasing and a size cutoff; the `GlyphRenderer` trait leaves room for curve rendering |
| wasm size | explicit features, `opt-level = "z"`, LTO, `wasm-opt -Oz`, a size gate |
| WebGL2 limits | no storage buffers in custom shaders |
| `f32` rendering against `f64` semantics | semantics only in the core, in `f64`; the studio converts for drawing and sends positions back only as edits |
| long build times | dependencies at `opt-level = 3` in dev, `dynamic_linking` in dev builds, a fast linker |
