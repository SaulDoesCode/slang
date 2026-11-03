# s³ 0.3

s is a right-aligned postfix language. Its one idea is the lane: a line of s splits into lanes, each lane runs the same words on its own line and its own copy of the names, and the lanes come back together at a join, filed by index. This repository is the s machine (Rust, and JavaScript for the page), its compiler to Rust, its standard library, the editors that draw lanes, and the places lanes are used.

## The line

A line of s is a sentence built and then consumed by a command. Words push onto the line from left to right: a number, a text in braces, a name, the value of a name (`x~`). A command consumes the whole line as its arguments, in order, and the line is empty after it.

```
5 x n
x~ 2 + =~ >
{a b c} xs /l
```

- `5 x n`: `n` takes the line `5 x`, and the name x holds 5.
- `x~ 2 + =~ >`: `+` takes 5 and 2 and writes 7 to the name `=`; `=~` puts 7 on the line, and `>` prints it.
- `{a b c} xs /l`: `/l` takes the text and the name, and the list xs holds a, b and c.

`a b c xs l` appends a, b and c to the list xs: every value before the list name is an entry.

A command word stands for the command wherever it appears. A name that is also a command goes on the line in braces: `5 {r} n` makes the name r hold 5, where `5 r n` runs `r` first.

## Lanes

A fork is a line with lanes in it: `⟪a¦b¦c⟫ rest ⨝`. Each lane's line is the line at the fork followed by that lane's beads and then the shared beads after `⟫`. `⟪xs…⟫` is a spread fork, one lane per item of the list xs, each lane's line starting with its item.

A fork is a write log. Each lane writes into a copy of the names as they were at the fork, and its writes are filed under its index. Every lane starts with the yes-flag `ow` as it was at the fork. At the join `⨝` the names are back to their state at the fork and the lanes are held, lane 0 first, until the next fork:

- `collect` puts every lane's line on the line, lane after lane; `xs into` makes the list xs from them.
- `i lane` reads lane i: its line comes back, and its writes are read first until `endlane`.
- `{code} reduce` folds the lanes' lines with code; `{code} lanewise` runs code once in each lane.
- `lanes` is how many there are.

```
{1 2 3 4 5 6 7 8} xs /l
⟪xs…⟫ 2 * =~ 1 + =~ ⨝ ys into ys o l/ o~ >
⟪1¦2¦3⟫ 10 * =~ ⨝ collect >
```

The first fork has a lane for each item of xs; each lane doubles its item and adds one, and `ys into` files the lanes' lines as the list ys, so the line prints 3 5 7 9 11 13 15 17. The second fork's lanes start with 1, 2 and 3, and `collect` prints 10 20 30.

Forks with the same label zip: their lanes pair by index. Forks with different labels multiply. A line with forks mints the plain lines it stands for, one per lane, and the editors show them (F6). BEADS.md is the specification; s5, its 5-bit text encoding, carries the bead marks as codes of their own.

## How a fork runs

The machine runs a fork one of three ways, and the editor labels each fork with the way it ran and how long it took:

- **A vector program.** When the lanes hold numbers and the shared beads are arithmetic, comparisons and conditional writes, the fork is one program over columns of numbers: comparisons are masks, a conditional write is a select. The machine runs it in blocks of 2048 lanes; the compiler to Rust writes it as a kernel over AVX-512, AVX or SSE2 registers, chosen on the machine it runs on. On ARM the kernels are NEON; the lane tests pass on aarch64 under qemu at the NEON and scalar levels, in the machine and compiled. It prints exactly what the lane-by-lane run prints, so it uses no fused multiply-add.
- **Threads.** When the lanes hold code, each lane runs on a thread with a snapshot of the names. The snapshot is kept while nothing is written between forks, and lists of numbers cross to the threads shared, without a copy.
- **One lane at a time.** Everything else.

Measured on this machine (`cargo test --release --test bench_lanes -- --ignored`, and `core/tests/lanes.rs`):

| | |
|---|---|
| a fork over ten million numbers, vector program in the machine | 58 ms |
| the same fork, compiled to Rust | 39–43 ms |
| the same fork, one lane at a time | 9.6 s |
| `into` from that fork | no copy: the list holds the column |
| a thread fork of 3 lanes with a million-item list in scope, after the first | 0.4–0.6 ms |
| a build in the editor (`s3 build`): the program with the runtime's source, first build of a text | 9 s |
| the same, for a program folded whole | 0.4 s |

At each join the editor shows the lane count and the lines of the first 16 lanes, from the machine or from the compiled program.

## Folding

A build runs the program at build time up to the first op that reaches outside the machine: the shell, stdin, files, threads, the network, random numbers, or the arguments. The binary starts at that op, with what the program printed before it already written and its names, lists, scripts, replacements and line in place, so a program that never reaches outside builds to its output and how it ended. A fork on threads runs one lane at a time during folding, so a lane that reaches outside is met there. `--no-fold` (or the editor's **fold** switch) builds the program as written. `core/tests/fold.rs` runs folded and unfolded builds of the same programs against the machine, with arguments and stdin.

## Where lanes are used

- **The machine and its compiler.** `s3 run`, `s3 build` and `s3 rs` run an s program, build it as one Rust program, or write the Rust. A built program prints how each fork ran when `S_FORKS` is set. `examples/lanes.s` has a fork of each way; `S_FORKS=1 s3 run examples/lanes.s` names the way each ran. `examples/blodra.s` is the cube journey: a state is a `side.way` name, the moves `top left bottom right` (and `A B C D`) are replacements, and `{ABCD} {f} walk show` walks a path from the front and prints the path, the state's four neighbours and the four peeks; `core/tests/blodra.rs` holds it to 300 walks of the JS.
- **The editor at `/`.** The files of a folder in tabs, words coloured as the machine reads them, runs on the machine (Ctrl+Enter) with what each line printed beside it, builds to Rust (Ctrl+Shift+Enter), definitions across loaded files and std, uses, completion, the palette, find and replace, arguments and stdin, a console. A fork is a box of rows, one per lane; the minted lines (F6) take edits that go back into the beads; braces are zones with an edge shape per kind. STUDIO.md has every key.
- **The page.** `studio/web/` is s for the page: the machine in JavaScript with lanes and `try`, words that describe a page, and a compiler that writes the page as HTML and CSS. A page that changes carries a script with only the parts of the machine it reaches; the counter's script is 4.7 KB gzipped. `studio/web/README.md` has the rest.
- **The room.** An s file as pages of text in space, flown through first person; F5 runs the file on the same machine, forks included.
- **The worlds studio.** Bricks, spheres, zones and datum cubes, on the same s machine.

```
cargo build --release
./target/release/s3-studio --files ~/s     # the editor on http://127.0.0.1:7333, the room at /room.html, the page at /web/
./target/release/s3 run prog.s             # run an s program
./target/release/s3 build prog.s           # an s program as one Rust program, compiled
./target/release/s3 rs prog.s              # the Rust only
cargo test --release
```

## The room

Every paragraph of a file is a page; W A S D, Space, C fly, Q E roll, the wheel moves in and out along the view and sideways, Tab captures the mouse. Double click a page to edit it in the box below; F5 runs the file and writes what each word printed beside its line; G goes to a definition or into a loaded file, B comes back; F1 lists every key. Where pages stand is kept beside the file as `.name.s.room`, in s. STUDIO.md has the rest.

The standard library, loaded with `std/name.s.` (or everything general with `std/all.s.`):

- `core` — drop dup swap over rot nip tuck clear depth · add sub mul div mod pow neg inc dec abs min max · lt gt le ge eq ne same differ not yes no · call if when unless test and or dip keep bi · times while until
- `line` — the line as an array: gather spill show count · map filter fold foldr scan · rev sum product most least iota til take skip sort
- `text` — cat concat glue joinwith quote · words chars lines split len reversed repeat squish · has subst occurs · upper lower · format printf say · records: get set keys
- `math` — frac floor ceil round trunc places · sign clamp within lerp sqrt sq hypot pi · even odd gcd lcm fact fib divisors prime range · mean median · rand dice seed
- `check` — expect expectn assert checked: each check prints ok, or what it got against what it wanted, beside its line in the room
- `sh` — pipelines over text lines: cat sh grep grepv sed head tail wc uniq nl shout put >file >>file
- `gem` — ord red rord gem ords
- `logic`, `datum` — words for the worlds studio

Docs: `BEADS.md` lanes and s5, `SPEC.md` the language and runtime, `STUDIO.md` the room, the server and the worlds studio, `WORLDS.md` the 0.3 design of the worlds, `HAUL.md` the plan this version of lanes was built from, `BEVY.md` a design for a Bevy studio.

## The worlds studio

A spatial programming system built on s: programs are columns of bricks read bottom to top, quotations are boxes you can walk into, definitions sit on shelves whose height is their generation, and spheres linked by rays carry whole lines between bodies of code. Zones are closures that capture names with Rust's modes.

- `SPEC.md` — the language and runtime
- `STUDIO.md` — the server API and the studio
- `BEVY.md` — the design for a later Bevy studio
- `WORLDS.md` — 0.3: the walkable world, scopes, primitives, multiplayer

```
cargo build --release
./target/release/s3-studio            # the worlds studio on http://127.0.0.1:7333/studio.html
./target/release/s3 run prog.s        # s programs, as s 1.0 ran them
./target/release/s3 run world.json    # a world, headless; thread zones on threads
./target/release/s3 serve 127.0.0.1:7400 world.json
                                      # host a world's net zones and exported gates
./target/release/s3 build world.json  # a world as one Rust program, compiled
./target/release/s3 import prog.s     # an s program as a world
cargo test --release                  # the machine, the s 1.0 corpus, worlds, zones,
                                      # threads, net zones over TCP, scopes, datum cubes
node studio/test/walk.test.mjs        # walking physics
node studio/test/datum.test.mjs       # tiles, rays, level of detail, the gimbal
```

0.3 so far: the grid layer of devices, wires, masts and assemblies beside the sphere layer; the world as s text; windows, themes, the menu and the tutorial; walking with the hotbar; scopes with the logic dialect; datum cubes with the gimbal, their schema as names, and the mutation log.

Crates: `core` (std only: machine, compiler, world, sessions, zones, partitions), `cli` (`s3`), `server` (`s3-studio`, Salvo). The studio is `studio/` (Three.js, opentype.js, IBM Plex Mono; vendored). Levels are in `levels/`.
