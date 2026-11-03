# The haul

Parts 1 to 7 are done; each is a commit. Measured results: part 1's fork over ten million lanes runs in 58 ms in the machine and 39–43 ms compiled, where the 30 ms target sat below the cost of a fresh ten-million-number column on this machine; `into` copies nothing. Part 3's build takes 0.4 s. Part 4's thread fork starts in 0.4–0.6 ms. Part 6's counter script is 4.7 KB gzipped.

One idea runs through everything: a line that splits into lanes and comes back together. This haul makes that idea fast on real data, wide enough for real code, and quick to build. The editor and the page come after the machine. Each part ends in a commit with its tests passing.

## 1. Numbers stored as numbers

A list keeps its items as `Vec<Val>`, 24 bytes each. A fork over ten million numbers spends most of its 145 ms converting them into a column, and `into` spends another 185 ms converting them back.

- A list becomes `enum List { Vals(Vec<Val>), Nums(Vec<f64>) }`. A list whose items are all numbers is stored as `Nums`. It turns into `Vals` the first time something that isn't a number goes in.
- Every list command (`l -l [l] l= l- #l /l l/`), `each`, `set_argv`, the lane snapshot and the compiler's state all read and write through the enum. What s programs see doesn't change.
- A spread fork over a `Nums` list reads it in place. `into` writes `Nums` straight from the columns.
- Done when: the corpus and every lane test pass unchanged, and `bench_lanes` shows the fork over ten million lanes under 30 ms, with `into` under 60 ms on this machine.

## 2. Wider vector programs

**Comparisons and conditionals.** `== << >> <<= >>= = !=` give a mask per lane, and `!!` flips it. A conditional form `x?` / `x!` whose inner op the vector machine has becomes a select: both sides are computed and the mask picks per lane.
- This holds when the inner op writes a name or pushes a value. A conditional call, return or print still runs lane by lane.
- The analysis tracks `ow` as a mask value. The kernels gain `cmp` and `select` (AVX `_mm256_cmp_pd` and `_mm256_blendv_pd`, SSE `_mm_cmp*_pd` with and/andnot, NEON `vcgtq_f64` and `vbslq_f64`).

**AVX-512.** An `Avx512` vector of 8 lanes, used when the machine has `avx512f`, alongside AVX and SSE2. This machine may not have it; the detection path is tested by forcing each level in a test build.

**No FMA.** A vector program must print exactly what the same fork prints lane by lane (BEADS §5.11). FMA rounds `a*b+c` once where the lane-by-lane run rounds twice, so a fused multiply-add would break that. BEADS §3.4 will say so.

Done when: §5.11 covers forks with comparisons and conditionals, at every vector level the machine has, and BEADS §3.4 lists the new ops.

## 3. Builds in seconds

A compiled program currently carries the runtime's source text, so rustc compiles the whole runtime on every build.
- s3-studio builds `s3-core` once as an rlib. A generated program `use`s it and holds only its chunks, forks, kernels and tables.
- The `build` request passes `--extern s3_core=…` and `-L` to the rlib's folder. `rust {path, text}` keeps writing a single-file program, for running away from the studio.
- Done when: a first build of `lanes.s` takes under 5 s, and the compiled test in `core/tests/lanes.rs` runs on both outputs.

## 4. Threads without copying everything

A thread fork currently converts the whole machine state for every fork.
- The snapshot is kept with the write epoch it was taken at, and reused while no name has been written since.
- Lists inside it are `Arc`-shared slices: with part 1 in place, a `Nums` list crosses as `Arc<[f64]>`, with no copy.
- Done when: a thread fork of 3 lanes with a million-item list in scope starts in under a millisecond after the first fork.

## 5. The editor

- **Mint view takes edits** (BEADS §4.4): an edit inside a shared bead changes every minted line; an edit inside a lane changes the lines that pass through it; an edit that makes one minted line differ inside a shared bead splits that bead into a new fork.
- **The native machine reports joins:** each join's lane count and the lines of its first 16 lanes, shown beside the join as the page playground does.
- **Commands where names belong:** a one-letter command word (`n r w l f t`) standing where the next command takes a name (`x r n`, `v w l`) gets a mark and a note saying `{r}` is the name (`\r` runs the command too).
- **Zones readable without colour:** each kind of zone also has its own edge shape (element square, event round, try double, loop dashed, page tall). The braces toggle stays.
- Done when: `test/editor.py` and `studio/test/s_editor.py` cover each of these.

## 6. The page runtime, smaller

The machine a compiled page carries is 12.6 KB gzipped.
- The commands become one function each, and the compiler emits only the ones its code can reach (a dispatched text that names a command counts as reaching it).
- The page words get the same treatment, and so does the random number generator, which only `#prng` and `` `prng `` reach.
- Done when: the compiled counter's script is under 5 KB gzipped and `test/compile.py` passes.

## 7. The README

The README leads with lanes as the one idea, in plain sentences: how the line works (a sentence built and then consumed by a command), how a fork works (the write log, lanes filed by index), and how a fork runs (vector program, threads, one lane at a time, as the editor reports). The other parts (the room, the page, s5, worlds) follow as places the idea is used. The numbers from `bench_lanes` go in as measured.

## Order

Parts 1 → 2 → 3 → 4 are the machine and its builds, each a commit. Then 5, then 6, then 7. If the session runs short, it stops after a finished part. Nothing is left half-built.
