# s for the page

s describes a page, and the page is built from the description. You can use it three ways:

- **The playground**, `index.html`: write s on the left and see the page on the right, live.
- **The compiler**: it turns a program into an HTML page that has no reader in it.
- **The interpreter**, `s-web.js`: it runs `<script type="text/s">` blocks when a page loads.

## Files

- `s.js`: the s machine, ported from `core/src/rt.rs`. It adds bead lanes (BEADS.md §3) and `try`.
- `beads.js`: reading and minting bead lines, and mutualize.
- `s5.js`: the 5-bit encoding (BEADS.md §1).
- `std.js`: the standard library as text, generated from `core/std`.
- `dom.js`: the page words, and the rule that makes a brace run again.
- `compile.js`: the compiler.
- `ed.js`: the editor.
- `play.js`, `play.css`, `index.html`, `frame.html`: the playground and its preview. `ed.css`: the editor's styles, shared with the native editor at `/s/`.
- `build.mjs`: run with `node build.mjs`. It writes the following, plus `.min.js` versions when esbuild is installed (`npm i`):
  - `s-web.js`: the interpreter, the library and the page words in one classic script.
  - `s-core.js`: the machine a compiled page carries, whole.
  - `s-core.parts.json`: the same machine cut into parts by `parts.mjs`: each top-level declaration, each method of `Rt`, each command, each page word and each kind of record a page brings back. `shake.js` puts back together the parts a page reaches.
- `examples/`: `first`, `counter`, `clock`, `site`, `lanes`, `table`, to open with **open**.
- `test/`:
  - `corpus.mjs`: the s corpus.
  - `beads.mjs`: BEADS §5 and `try`.
  - `page.py`: the page words in a browser.
  - `compile.py`: compiled pages in a browser.
  - `editor.py`: the editor in a browser.
  - `npm test` runs the first three.

## Elements

- `{children} div.card#main` makes an element. The word is the tag, then classes (camelCase becomes kebab-case) and an id. `<div` is the same word.
- The brace before the element word is its children. The brace runs as s when it holds any of these:
  - a brace, a quote or a lane mark;
  - an element word written with a class, an id or an angle bracket;
  - a `$selector`, an attribute, an event or a component.

  It also runs as s when its last word is an element word of two or more letters. Otherwise it is text.
  - A sentence that ends with an element name stays text if it ends with a period.
- A name spelled like an element is written with a backslash: `x \a l` adds x to list `a`.
  - The library defines `div`, `sub` and `map` as maths words. Inside the library's own words they stay maths; everywhere else they make elements.
- Attributes and events come before the element they shape: `{/about} href=`, `{n;} onclick`.
  - While an event runs, `e.value~ e.checked~ e.key~ e.id~ e.text~` hold what happened.

## Names the page follows

While an element's brace runs, the machine records the names it reads. When an event, a timer or an input writes one of those names, that brace runs again. Only that element is rebuilt: the innermost element that read the name, or the outermost one when an element and an element inside it both read it.

- `{~n~} p`: a text with names. The element follows the names.
- `{~url~} href=`: an attribute made from names follows them.
- `{~draft~} value= input`: an input whose value is exactly one name holds that name both ways. `checked=` on a checkbox works the same way.
- `refresh` runs every brace that read a name.

## Lanes and try

- `⟪1¦2¦3⟫ 10 \+ =~ ⨝` splits the line into three lanes. Each lane runs the rest of the line up to the join; then the lanes join.
- After the join, these words work on the lanes: `lanes`, `i lane`, `endlane`, `collect`, `{q} reduce`, `{q} lanewise`.
- The ASCII marks `[[ || ]] ||>` are the same as `⟪ ¦ ⟫ ⨝`.
- `⟪xs…⟫` has one lane per item of list `xs`, the item on the lane's line; `ys into` puts every lane's line into list `ys`.
- In the page, every fork runs one lane at a time. The native machine (`/s/` in s3-studio) runs forks over numbers as SIMD and forks over code on threads.
- `{body} {handler} try` runs body. When an error happens in body, it runs handler, with `err~` holding the message.

## The compiler

`.html` in the playground compiles. The program runs once at build time, in a page of its own, and the elements it built are written out as HTML. The style rules it made go into a `<style>`.

- A page with no events, timers, two-way inputs, kept names or second page has no script at all.
- A page that changes also gets one script. That script holds:
  - the code of its events, timers, pages and following braces, as JavaScript functions;
  - the library words that code reaches, also as functions;
  - a snapshot of the names;
  - the parts of the machine that code reaches: the commands it names, the page words it uses, the kinds of records it has (events, regions, inputs, pages, kept names, timers), and the methods all of those call. The random number generator comes only with `#prng` or `` `prng ``, the lanes only with a fork, the one-word reader only with code that reads a name as a word, and the whole reader with code that runs a text (`~`, `?)`, `><`).

  The program's top level, the reader and the library text stay behind.
- s can run text as code with `~`, `?)` and `><`. Every brace written in the program is compiled. A page whose code uses one of those three also carries the reader, every command and every page word, so text it builds as it runs is read and runs on the page.

| example | compiled | gzipped | script |
|---|---|---|---|
| first | 464 B | 324 B | none |
| table | 55.8 KB | 12.2 KB | none |
| counter | 12.1 KB | 4.9 KB | 2 functions; the script is 4.7 KB gzipped |
| site | 34.8 KB | 11.3 KB | 29 functions, 3 library words |

`.html + reader` writes the program text with the whole machine and library (22 KB gzipped) instead.

## The editor

- **Zones.** Braces are drawn as coloured zones. The colour comes from the word that takes the brace: element, component, attribute, event, `try` (red, its handler a darker red), loop, page, style, definition, comment.
  - Each zone has an edge at both ends, so the caret can stand inside or outside the zone. The edge has a shape for each kind, so zones read without colour: element square, event round, `try` double, loop dashed, page tall, definition notched, style dotted.
  - `braces` shows the brace characters.
  - Typing `{` makes an empty zone; with a selection, it wraps the selection.
- **Middle-drag.** A pointer runs from where the drag began, labelled with the length and with what the range will become. The wheel picks what: try, element, on click, each, every, page, word, css, group or lanes. Releasing wraps the range, grown to whole words, zones and forks.
- **Middle-click** on a zone opens a menu to unwrap it or retype it.
- **Lanes.** A fork is drawn as a box of rows, one row per lane. The beads after the merge are shown faintly on the other rows. Up and Down move between lanes.
  - After a run each join shows the lines its lanes left. In the native editor at `/s/`, the run also labels each fork with how it ran (vector program, threads, or one lane at a time), and each join shows its lane count and the lines of its first 16 lanes.
- **Commands where names belong.** A one-letter command word (`n r w l f t`) right before a command that takes a name from it, as `r` in `5 r n`, has a wavy underline: `r` runs there. Its note says `{r}` is the name: braces push the text r, and the command reads it as a name. `\r` runs the command too.
- **Keys:**

  | key | does |
  |---|---|
  | Ctrl+\ | split the selection into lanes |
  | Alt+Enter | add a lane |
  | Alt+Backspace | remove a lane |
  | Ctrl+] | merge |
  | Ctrl+Shift+] | join |
  | Ctrl+L | label a fork |
  | Ctrl+M | fold the selected lines into one bead line |
  | F6 | show the minted lines of the caret's line, each one editable (below) |
  | Ctrl+Enter | run |
  | Ctrl+S | save `.s` |
  | F1 | the words |

- **Minted lines (F6).** Each line the caret's bead line mints is a row that takes edits (BEADS §4.4). An edit goes to the beads it came from: inside a shared bead it changes every minted line, inside a lane it changes the lines through that lane. With **this line only** checked, the words the edit touched become a new fork whose lanes zip with the line's fork, so the other lines keep them; where no fork zips, the line is folded again from its minted lines (Ctrl+M).
- **Files:** **open** (Ctrl+O) and **save** (Ctrl+S) work on the file itself where the browser allows it (Chrome, Edge); elsewhere save downloads. **new** starts an empty program. The name shows a dot while there are unsaved changes.
- **Status line:** the caret line's forks, lanes and minted lines, and the size in UTF-8 and s5. `.s5` saves the program in s5.
