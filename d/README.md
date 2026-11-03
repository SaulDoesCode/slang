# d

`$` builds DOM from abbreviations and keeps it live with signals. It is one file with no dependencies: `d.js` as a module, or `d.min.js` as a classic script that sets `window.$` (9.6 KB, 4.0 KB gzipped).

```js
n = $(0)
$`div>h1${n}+button{more}${() => n(x => x + 1)}`
```

## 1. `$`

1.1 `` $`…` `` (a tagged template) reads the abbreviation and builds it. One top element is returned as that element; several are returned as a DocumentFragment. Either one carries `.$`, the refs (§2.6).

1.2 `$(v)` with any other argument makes a signal holding v.

1.3 A template is read once per call site, and every later call builds from that reading.

## 2. Abbreviations

2.1 Grammar (whitespace between terms is free; inside `{ }` and `[ ]` it is text):

```
seq     = term { ("+" | ">" | "^"+) term }
term    = ( element | "(" seq ")" | SLOT ) [ "*" count ]
element = [tag] { "." name | "#" name | "[" attrs "]" | "{" text "}" | "@" name | ":" value | "*" count | SLOT }
count   = digits | SLOT
attrs   = { name [ "=" ( value | '"' text '"' ) ] }
```

2.2 Structure:
- `a>b`: b inside a.
- `a+b`: b after a.
- `^`: back up one level, once per caret.
- `( … )`: a group.

2.3 An element with no tag is a `div`, except:
- in `ul`, `ol` and `menu` it is `li`;
- in `table`, `thead`, `tbody` and `tfoot` it is `tr`;
- in `tr` it is `td`;
- in `select`, `datalist` and `optgroup` it is `option`;
- in `dl` it is `dt`.

2.4 Attributes and text:
- `.c` adds a class and `#i` sets the id.
- `[k=v k2="v w" k3]` sets attributes; one with no value is set present.
- `{t}` adds text.

2.5 `:v` sets the element's main attribute, which depends on the tag:

| main attribute | tags |
|---|---|
| `href` | `a`, `link`, `area`, `base` |
| `src` | `img`, `script`, `iframe`, `video`, `audio`, `source`, `track`, `embed` |
| `type` | `input`, `button` |
| `action` | `form` |
| `for` | `label`, `output` |
| `value` | `option`, `data` |
| `content` | `meta` |
| `datetime` | `time` |
| `data` | `object` |
| `colspan` | `td`, `th` |
| `start` | `ol` |
| `placeholder` | `textarea` |
| `name` | `select` |
| `title` | any other tag |

The value runs to the next space or to one of `{ } [ ] > + ^ * ( ) @`.

2.6 `@r` records the element in `.$.r`. On `input`, `textarea`, `select` and `button`, `@r` is also the element's `name`, unless the element already has one.

2.7 Repeats:
- `*3` builds the term three times, and `$` is the count, 1 to 3.
- `*${items}` builds it once per item. `$` is the item, and `$.a.b` reads the item's fields.
- `$` works in text, classes, ids, attribute values and main values. `$$` is a dollar sign.

## 3. Slots

3.1 Right after an element (in its parts, with no `+` or `>` between):
- **A function** is a handler for the element's natural event, and is called with `(e, element)`, or `(e, item, i)` inside a repeat. The natural event is:
  - `submit` on `form`: the page stays, and the handler is called with `(e, values, form)`, where values are the form's fields by name;
  - `change` on `select` and on checkbox and radio inputs;
  - `input` on other inputs and on `textarea`;
  - `click` on everything else.
- **A signal** on `input`, `textarea` or `select` holds the field's value both ways (`checked` for checkbox and radio). A signal holding a number reads the field back as a number. On any other element, a signal is live text.
- **An object** is attributes:
  - a key starting with `on` and holding a function is an event;
  - any other function value is live;
  - `class: {name: flag}` toggles classes, and `style: {prop: v}` sets styles;
  - `value`, `checked`, `disabled`, `selected`, `hidden`, `textContent` and `innerHTML` are set as properties.
- **A node, an array, a string or a number** is children.

3.2 In `{text}`, `[k=…]` and `:…`: a value is used as it is, and a signal or a function makes the text, attribute or main value live. A function in `[on…=${f}]` is that event.

3.3 In place of an element:
- a node, a string, a number or an array goes in as it is;
- a function is a region, rebuilt whenever what it read changes;
- `*${signal}` over an array is a live list: an item still in the array keeps its elements and their state, a new item is built, and a removed one goes along with its effects.

## 4. Signals and effects

4.1 A signal `s`:
- `s()` reads it, and inside an effect the read is recorded;
- `s(v)` writes v;
- `s(f)`, with f a function, writes `f(old)`;
- `s.peek()` reads without recording.

A write runs the effects that read the signal, when the value changes or is an object.

4.2 `$.fx(f)` runs f now, and again whenever a signal it read is written. An effect made while another runs belongs to it: it is dropped when its owner runs again or goes. Regions, live text, live attributes and list items are effects of this kind.

4.3 `$.c(f)` is a signal that follows `f()`.

## 5. The rest

- `` $.css`…${v}…` ``: a style sheet; a signal or function in a slot keeps it live.
- `$.keep(name, s)`: s is loaded from localStorage under `$:name` and saved on every write.
- `$.hash`: a signal holding the address after `#`.
- `$.at(sel, …nodes)`: appends to the element sel names (or an element given, or the body).
- `$.q(sel)`, `$.qa(sel)`: querySelector, and querySelectorAll as an array.

## 6. Size

Characters of abbreviation against characters of the HTML built (`test/d.py`):

| abbreviation | chars | HTML | × |
|---|---|---|---|
| `ul>li.item$*3{Item $}` | 21 | 96 | 4.6 |
| `table>tr*10>td*10{$}` | 20 | 1115 | 55.8 |
| `nav>a:#/{Home}+a:#/about{About}+a:#/list{List}` | 46 | 84 | 1.8 |
| `form>label:n{Name}+input@n+label:m{Mail}+input:email@m+button{Send}` | 67 | 133 | 2.0 |

The HTML counts leave out what the abbreviations also carry: the events, the bindings and the submit that hands over the values.

## 7. Files

- `d.js`: the library. `node build.mjs` makes `d.min.js`, using esbuild from `studio/web`.
- `index.html`, `play.js`, `play.css`: the playground.
  - Code on the left; the page it makes on the right. When the code's last value is a node, the page shows it.
  - The counts below the code: characters of code, characters inside `` $`…` ``, and the page's elements and characters of HTML.
  - Open and save write the file itself where the browser allows it; `.html` writes the page as one file. F1 has the abbreviations.
- `test/d.py`: the library in a browser, with 42 checks.
- `test/play.py`: the playground, with 5 checks.
