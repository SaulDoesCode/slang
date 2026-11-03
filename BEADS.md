# s5 and beads

Three parts: a 5-bit text encoding (s5); bead lines, where a line of text splits into lanes and rejoins; the meaning of bead lines in s, where a split that rejoins is a SIMD operation or a set of threads. A fourth part specifies an editor for bead lines.

## 1. s5: 5-bit text

1.1 A text is a sequence of 5-bit codes. Codes are packed most significant bit first into bytes. A stream begins with the number of codes as an unsigned LEB128 varint; bits after the last code are zero.

1.2 The letter table L is the default table:

| code | meaning |
|---|---|
| 0 | space |
| 1–26 | a–z |
| 27 | CAP |
| 28 | FIG |
| 29 | SYM |
| 30 | UNI |
| 31 | BEAD |

1.3 CAP makes the next letter uppercase. CAP CAP makes every letter of the next token uppercase; a token ends at the first code that is not a letter.

1.4 FIG reads the next code from the figure table F. FIG FIG enters figure lock: codes are read from F until code 31, which returns to L. Inside figure lock, code 30 is UNI. Because code 28 after a single FIG enters lock, `#` (F code 28) is written inside figure lock.

| F code | 0–9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24 | 25 | 26 | 27 | 28 | 29 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| char | 0–9 | `.` | `,` | `;` | `:` | `!` | `?` | `'` | `"` | `-` | `(` | `)` | `{` | `}` | newline | tab | `/` | `~` | `@` | `#` | `=` |

1.5 SYM reads the next code from the symbol table Y: `<` `>` `+` `*` `&` `$` `%` `[` `]` `\` `|` `^` `_` `` ` `` as codes 0–13; codes 14–30 are reserved.

1.6 UNI is followed by a code n, 1 ≤ n ≤ 5, then n codes holding a Unicode scalar value, most significant group first. Every character outside L, F and Y is written with UNI.

1.7 BEAD is followed by one code naming a bead mark (§2.9).

1.8 Decoding is total: every code sequence produced by an encoder decodes to the text it encoded. Encoders choose between FIG and figure lock; the reference encoder enters lock before a run of three or more figure characters.

1.9 Measured on texts in this repository with the reference encoder, `studio/web/s5.js` (UTF-8 bytes against s5 bytes):

| text | UTF-8 | s5 | ratio |
|---|---|---|---|
| `README.md` (prose) | 4562 | 3211 | 70% |
| `studio/web/examples/site.s` | 1949 | 1494 | 77% |
| `core/std/math.s` | 3401 | 2679 | 79% |
| `studio/web/dom.js` | 19847 | 15363 | 77% |
| `core/std/core.s` | 5281 | 4417 | 84% |
| `core/std/line.s` | 3941 | 3468 | 88% |

Lowercase prose approaches 5/8 of UTF-8. s code sits higher because braces, sigils and figures cost two codes outside figure lock. An s5 stream can be entropy-coded afterwards; s5 comes first.

## 2. Bead lines

2.1 A bead line is a line of text with structure marks. A bead is a maximal run of text containing no mark.

2.2 Marks:

| mark | text form | ASCII form | meaning |
|---|---|---|---|
| FORK | `⟪` | `[[` | the line splits into lanes |
| LANE | `¦` | `\|\|` | the next lane begins |
| MERGE | `⟫` | `]]` | the lanes stop differing; each lane continues with the beads that follow |
| JOIN | `⨝` | `\|\|>` | the lanes become one line again |
| LABEL | `⟪name:` | `[[name:` | the fork belongs to lane set `name` |

2.3 Between FORK and MERGE stand k ≥ 1 lanes separated by LANE. The beads between MERGE and the next JOIN (or the end of the line) are shared: each lane continues with them.

2.4 A fork's lanes belong to a lane set: the set named by its LABEL, or the line's default set when it has none. Forks in one set zip: lane i of every fork in the set belongs to lane i of the set. Forks in one set have the same number of lanes, or one lane, which every lane of the set shares. Forks in different sets multiply: the lanes of the line are the product of its sets.

2.5 A fork at the start of a line gives the line several starts. A fork with no JOIN after it gives the line several ends. A JOIN with no open fork is an error.

2.6 Lanes contain bead lines: forks nest. An inner fork joins before its outer lane ends, or its lanes become lanes of the outer lane.

2.7 The expansion of a bead line is the set of plain lines it mints: for each choice of one lane per set, the line made of the shared beads and the chosen lane of every fork, in order. A line with sets of sizes k₁ … kₘ mints k₁ × … × kₘ lines. A JOIN mints nothing; in text, the beads after it are shared by every minted line.

```
the ⟪cat¦dog⟫ sat on the ⟪mat¦log⟫
  → the cat sat on the mat
  → the dog sat on the log

the ⟪a:cat¦dog⟫ sat on the ⟪b:mat¦log⟫
  → the cat sat on the mat     the cat sat on the log
  → the dog sat on the mat     the dog sat on the log

⟪we¦they⟫ ran ⨝ home
  → we ran home
  → they ran home
```

2.8 A spread fork is a fork with one lane holding one bead `name…` (or `name...`): in s it has one lane per item of list `name` (§3.2a). In text it mints the bead as written. A spread fork is the only fork of its region.

2.9 In s5, BEAD is followed by: 0 FORK, 1 LANE, 2 MERGE, 3 JOIN, 4 LABEL; LABEL is followed by the label's letters and code 0.

## 3. Bead lines in s

3.1 At a FORK, the line is copied into each lane of the fork's set. Each lane runs its own lane bead, then the shared beads after MERGE. At JOIN, the lanes come back to one axis. Each lane keeps its line and its writes, and stays addressable by its index i, 0 ≤ i < k. The beads after JOIN run once, on the line as it stood at the FORK, with the joined lanes held beside it.

3.2 A lane reads names, lists, scripts and replacements as they stood at the FORK. Writes made in a lane (`@ # ; : $` and `><`, list words, arithmetic results such as `=`) are filed under that lane's index and outlast JOIN. Output a lane writes is held and written at JOIN in lane order.

3.2a A spread fork copies the line into one lane per item of its list, as the list stood at the FORK, and pushes the item on the lane's line; every lane then runs the shared beads.

3.3 Words on joined lanes:

| word | effect |
|---|---|
| `lanes` | pushes the number of joined lanes |
| `i lane` | puts lane i's line on the line; until the next `i lane` or `endlane`, reading a name finds lane i's write first |
| `endlane` | reading names finds the line's own writes again |
| `collect` | puts every lane's line on the line, lane 0 first |
| `name into` | puts every lane's line, lane 0 first, into list `name` |
| `{q} reduce` | puts lane 0's line on the line, then runs q once per further lane with that lane's line pushed after it |
| `{q} lanewise` | runs q once per lane, in lane order, with that lane's line on the line and its writes readable; the line q leaves becomes that lane's line, and q's writes are filed under that lane |

3.4 How a fork runs leaves its meaning unchanged:
- A fork runs as a vector program when every lane bead pushes numbers (the same count in every lane), or it is a spread fork over numbers, with an empty line at the FORK, and the shared beads hold only: numbers, lookups, `+ - * / ^ %`, `n`, `#`, `;`, `:`, `//`, the comparisons `= != == << >> <<= >>= & ;`, `!!`, and conditional forms (`x?`, `x!`) of `#`, `;`, `:`, `n`, `+ - * / ^ %`, lookups and numbers. Commands count when they have no definition or are escaped. A conditional runs in the lanes where `ow` says so, so a value it leaves on the line is there in those lanes only; the program stands when every lane ends with the same line. A name the shared beads write must hold no text at the FORK, and a name written in some lanes must hold a number at the FORK. The shared beads then run once over columns of k lanes, with SIMD; `ow` is a mask, a conditional write is a select.
- Otherwise a fork whose lane beads hold code runs its lanes on threads, each lane in its own machine with the state at the FORK; output is written at JOIN in lane order.
- Otherwise the lanes run one at a time.
Every lane starts with `ow` as it stood at the FORK, and JOIN puts it back. JOIN waits for every lane. The lanes are addressable by index in all three. A vector program prints what the same fork prints lane by lane, so it computes each operation as the lane would, with no fused multiply-add.

3.5 An error in a lane ends the program. The error names the lane, and the writes of the other lanes stay filed under their indices.

3.6 The compiler writes a vector program as one Rust loop over SIMD registers, in AVX-512, AVX and SSE2 versions on x86-64 and a NEON version on aarch64, chosen when the program starts; `S_SIMD` (avx512, avx, sse2, neon, scalar) narrows the choice. It writes every lane text as a compiled chunk, so lanes on threads run compiled code.

3.7 Examples:

```
⟪1¦2¦3¦4⟫ 10 \+ =~ ⨝ collect >
  → lanes 1 10 + =~ … each leaves 11 12 13 14 in its own =
  → collect puts them on the line: 11 12 13 14

⟪1¦2¦3¦4⟫ 10 \+ =~ ⨝ {\+ =~} reduce >
  → 50

⟪{a.txt} r a.txt~ lines¦{b.txt} r b.txt~ lines⟫ ⨝ 1 lane count >
  → with std/text.s loaded: two thread lanes read two files; the count of lane 1's lines

{~it~} ⟪h1¦h2¦p⟫ ⨝
  → in a page: one value, three elements; the lane beads hold code, so the fork runs as threads, in lane order in the browser
```

## 4. The editor

4.1 A bead line is drawn on one axis, right-aligned as s is. At a FORK, the axis splits into one track per lane, stacked; at MERGE, the tracks continue side by side with the shared beads repeated faintly on each; at JOIN, the tracks come back to one axis. A line with several starts or ends shows its tracks open at that edge.

4.2 The caret moves along beads. Inside a fork, Up and Down move to the same column of the neighbouring lane.

4.3 Keys:

| key | action |
|---|---|
| Ctrl+\ | split: the selection becomes a fork with the selection as lane 0 and an empty lane 1 |
| Alt+Enter | add a lane after the caret's lane |
| Alt+Backspace | remove the caret's lane |
| Ctrl+] | merge at the caret |
| Ctrl+Shift+] | join at the caret |
| Ctrl+L | label the caret's fork, putting it in a named set |
| F6 | mint view on and off |

4.4 Mint view shows the expansion as plain lines. An edit in mint view changes the bead line:
- An edit inside a shared bead changes every minted line.
- An edit inside a lane changes the lines that pass through that lane.
- An edit that makes one minted line differ inside a shared bead splits that bead into a new fork.

4.5 Mutualize: with several plain lines selected, the editor aligns them by tokens (the longest common subsequence of their tokens) and folds them into one bead line. Runs common to every line become shared beads, differing runs become lanes of one set, and differing starts become a fork at the start.

4.6 The status line shows the forks, lanes and minted lines of the current line, and the file's size in UTF-8 and in s5.

4.7 Running s: after a run, each join shows the lines its lanes left, and each fork shows how it ran (§3.4) as the machine reports it: the vector width and instruction set with the lane count, the number of threads, or one lane at a time. A fork that has not run shows nothing.

4.8 Files are plain text with the text-form marks, so a bead file opens in any editor. The editor also writes s5 (`.s5`).

## 5. Tests

5.1 s5 round trip: every text in `core/std`, `core/tests/cases` and `studio/web` encodes and decodes to itself.

5.2 CAP: `CAP h i` decodes to `Hi`; `CAP CAP h i space t` decodes to `HI t`.

5.3 UNI: `⟨` (U+27E8, 14 bits) encodes as UNI, 3, then three codes, and decodes back. The bead marks `⟪ ¦ ⟫ ⨝` encode as BEAD and one code.

5.4 Expansion: the three examples in §2.7 mint exactly the lines shown, in that order.

5.5 Zip error: `⟪a¦b⟫ ⟪c¦d¦e⟫` with no labels is an error naming the second fork; labelled `⟪x:a¦b⟫ ⟪y:c¦d¦e⟫` mints six lines.

5.6 SIMD: `⟪1¦2¦3¦4⟫ 10 \+ =~ ⨝ collect >` prints ` 11 12 13 14`; with `{\+ =~} reduce` in place of `collect` it prints ` 50`.

5.7 Writes filed by lane: `5 x# ⟪1¦2⟫ x# ⨝ x~ >` prints ` 5`; `5 x# ⟪1¦2⟫ x# ⨝ 1 lane x~ >` prints ` 2`.

5.8 Output order: `⟪{a} >¦{b} >⟫ ⨝` writes the line ` a` then the line ` b`, whatever order the lanes ran in.

5.9 Lanes after JOIN: `⟪1¦2¦3⟫ ⨝ lanes >` prints ` 3`; `⟪1¦2¦3⟫ ⨝ {2 \* =~} lanewise collect >` prints ` 2 4 6`.

5.10 Spread: `{1 2 3 4} xs /l ⟪xs…⟫ 10 \* =~ ⨝ ys into ys o l/ o~ >` prints ` 10 20 30 40`.

5.11 A vector program gives what the same fork gives lane by lane: each fork of 5.6 and 5.10, run once as it is and once with `0 \( \)` before the join (which keeps it lane by lane), prints the same.

## 6. Where it is implemented

6.1 `studio/web/beads.js` reads bead lines, mints them, and folds plain lines into one (§2, §4.5). `studio/web/s5.js` encodes and decodes s5 (§1).

6.2 The native machine, `core/src/lanes.rs` with `core/src/rt.rs`, implements §3, with the three ways of §3.4. x86-64 is tested at every level it has; aarch64 is tested under qemu at NEON and scalar (`cargo test --release -p s3-core --target aarch64-unknown-linux-gnu`, with `aarch64-linux-gnu-gcc` as the linker, `QEMU_LD_PREFIX=/usr/aarch64-linux-gnu`, and qemu-aarch64 registered with binfmt_misc so the tests' own builds run). The compiler, `core/src/compile.rs`, implements §3.6. `core/tests/lanes.rs` runs §5.6–§5.11, threads, `try`, and a compiled program; `core/tests/bench_lanes.rs` times one fork over ten million lanes three ways.

6.3 The web machine, `studio/web/s.js`, implements §3 with every fork running one lane at a time.

6.4 The editor, `studio/web/ed.js`, implements §4 in two places: the native editor at `/s/` in s3-studio, which runs files on the native machine and builds them to Rust and shows how each fork ran, and the page playground at `/web/`. In both, mint view (§4.4) shows the minted lines but does not take edits.

6.5 `studio/web/test/beads.mjs` runs §5.1–§5.10 on the web machine.
