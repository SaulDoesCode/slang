# s to Bend

s builds to Bend 2. Bend compiles the result to C (clang), to the GPU (CUDA, Metal) and to JavaScript.

## Method

The Bend program is what the s machine leaves to do when it runs the program at build time. The machine runs with every value either known or unknown, where an unknown is what comes from input, the outside, or a known bound being passed. Known work is done at build time. Each operation on an unknown value is written out as Bend. An s construct the run never reaches needs no Bend form.

- A script, a replacement body and a continuation after a call are each a Bend function. A jump is a tail call, which Bend emits as a `musttail` jump.
- Each name is a slot, and each word's meaning is fixed by generation at build time. A replacement that changes after input is guarded by its generation number.
- The line at each op is function arguments, because its depth is known there.
- Effects are `IO` `do` blocks in s order: print, files, process (`<`), arguments, TCP, time, random, spawn and channels. Reading stdin (`<_`) is a Bend effect that s adds, as a `.c` file and a `.js` twin.
- A fork is a fork-join parallel call over the lane range, and `!` sends it to the GPU. A lane is a pure function from the state at the fork to its line and its writes, and the join gathers the halves. `reduce` is a tree fold where a law states that its body is associative.
- A `law` compiles to a Bend law, checked by Bend's proof check. A law over a finite domain is also checked by running it at build time.
- Text built and run at runtime (`~`, `?)`, `><`) is a build error naming the site. That program builds through Rust.

## Numbers

- By default numbers are Bend's own. A name proven to hold whole numbers in `0..2^32` is `U32`; every other number is `F32`.
- A precision mode is an s feature, opted into: an exact software f64, and an arbitrary-precision integer (like BigInt), both built from `U32` in Bend. A name or fork in precision mode prints what the machine prints.

## Tests

Each test goes s → Bend → clang binary and is compared with the machine, at the precision the program asks for:

- the corpus cases that run no built text;
- `examples/blodra.s` with its 300 walks;
- the fold cases;
- `examples/lanes.s`, including the million lanes;
- the loads brought to the haul.

The CPU-parallel run is tested in the container. The GPU run is on a machine with a GPU.

## Where it stands

`s3 bend file.s -o out` builds an s program to Bend and from there to a binary (`-o out.bend`, `.c`, `.js` stop earlier). It needs `bend` on the PATH, or `S3_BEND` naming it (a `main.ts` runs under bun).

Built and the same as the machine:
- 25 of the 30 corpus cases, with files (`r r@ w +w mv`, handles `f +f @f f- f_ _f ?eof`), the shell (`<`) and the seeded random numbers (`#prng`, `` `prng ``, the machine's own generator);
- `examples/lanes.s`, its million-lane fork a map over the list, in parallel;
- `examples/blodra_in.s`, the cube walks read from stdin, all 300 walks;
- the input programs in `core/tests/bend` (`check.sh` runs them): sums from stdin, fib from the arguments (non-tail recursion, 13x the machine), words per line, a fork over numbers from stdin, a command interpreter on `?)`.

A pure fork over a list runs in parallel: the list cut once into runs of 4096 held in a balanced tree, the halves of each node a Bend parallel call.

Not yet:
- threads and tcp (`t -t <t >t *** tcp`): each is a build error naming its site;
- a line whose length comes from input (`std_sh`'s spill of a shell's lines);
- numbers are F32, so `tail.s`'s sum of a million and `std_math`'s random f64 differ in their digits; the precision mode is not built;
- lists are linked lists of boxed values: a fork over numbers runs at about the machine's speed on two cores, behind its SIMD on one.

## s3 native

`s3 native file.s -o out` writes the same residue as Rust and builds it with rustc. Numbers are f64, texts Arc<str>, lists Arc<Vec> moved on their last use and changed in place, lists of numbers Arc<Vec<f64>>. A function calling itself last is a loop; scripts calling each other last fold into one loop at build time. A fold over the residue (shared with the Bend writer) keeps only the parameters a function reads, the outputs a caller reads and the statements whose values are read, and passes V parameters read only as numbers or text unboxed. A worker message that does effects runs on its own thread from `<t` to `>t`; one that does none folds away. All 30 corpus cases, the input programs and the blodra walks match the machine. Bend matches 27: std_math and tail differ in F32, and rs_tcp needs a second thread, which Bend's IO does not have.
