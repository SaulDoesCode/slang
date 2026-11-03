// d.js — DOM from the fewest characters. One arrow, $:
//   $`ul>li.item*3{Item $}`   an abbreviation with ${slots} builds the DOM
//   $(0)                      a signal: n() reads, n(5) writes, n(x => x + 1) updates
// What a slot does follows where it stands; see README.md.

// ---------- signals and effects ----------

let cur = null // the effect that is running: it reads, it owns what it makes

const dispose = (e) => {
  if (e.dead) return
  e.dead = 1
  for (const d of e.deps) d.delete(e)
  e.deps.clear()
  for (const k of e.kids) dispose(k)
  e.kids.length = 0
}

const fx = (f) => {
  const e = { deps: new Set(), kids: [], dead: 0, on: 0, again: 0 }
  e.run = () => {
    if (e.dead) return
    if (e.on) { e.again = 1; return }
    e.on = 1
    do {
      e.again = 0
      for (const d of e.deps) d.delete(e)
      e.deps.clear()
      for (const k of e.kids) dispose(k)
      e.kids.length = 0
      const p = cur
      cur = e
      try { f() } finally { cur = p }
    } while (e.again && !e.dead)
    e.on = 0
  }
  if (cur) cur.kids.push(e)
  e.run()
  return e
}

const sig = (v) => {
  const subs = new Set()
  function s(x) {
    if (!arguments.length) {
      if (cur) { subs.add(cur); cur.deps.add(subs) }
      return v
    }
    const n = typeof x === 'function' && !x.$s ? x(v) : x
    if (n !== v || (n && typeof n === 'object')) {
      v = n
      for (const e of [...subs]) e.run()
    }
    return v
  }
  s.$s = 1
  s.peek = () => v
  return s
}

const isSig = (x) => typeof x === 'function' && x.$s === 1
const isFn = (x) => typeof x === 'function'
const plain = (x) => x && typeof x === 'object' && Object.getPrototypeOf(x) === Object.prototype

// ---------- reading an abbreviation ----------

// A template is a run of characters with slots between them: [char, char, {s: i}, ...].
const cache = new WeakMap()

function read(strs) {
  let ast = cache.get(strs)
  if (ast) return ast
  const src = []
  strs.forEach((s, i) => {
    for (const c of s) src.push(c)
    if (i < strs.length - 1) src.push({ s: i })
  })
  let p = 0
  const peek = () => src[p]
  const ws = () => { while (typeof src[p] === 'string' && /\s/.test(src[p])) p++ }
  const name = () => {
    let w = ''
    while (typeof src[p] === 'string' && /[\w-]/.test(src[p])) w += src[p++]
    return w
  }
  // text with slots and item references: $ the item (or the count), $.a.b its fields, $$ a dollar sign
  const parts = (stop) => {
    const out = []
    let t = ''
    while (p < src.length && src[p] !== stop) {
      const c = src[p]
      if (typeof c === 'object') { if (t) out.push(t), (t = ''); out.push(c); p++; continue }
      if (stop === ']' && (c === ' ' || c === '"')) break
      if (c === '$') {
        p++
        if (src[p] === '$') { t += '$'; p++; continue }
        const path = []
        while (src[p] === '.' && typeof src[p + 1] === 'string' && /[A-Za-z_]/.test(src[p + 1])) { p++; path.push(name()) }
        if (t) out.push(t), (t = '')
        out.push({ it: path })
        continue
      }
      t += c
      p++
    }
    if (t) out.push(t)
    return out
  }
  const fail = (m) => { throw new Error(`$: ${m} at ${p}: ${strs.join('${…}')}`) }

  function term() {
    ws()
    let n
    const c = peek()
    if (c === '(') {
      p++
      n = { t: 'g', kids: seq().list }
      ws()
      if (peek() !== ')') fail('a ( without )')
      p++
    } else if (typeof c === 'object') {
      n = { t: 's', s: c.s }
      p++
    } else {
      n = { t: 'e', tag: name(), cls: [], id: null, at: [], text: null, ref: null, main: null, mods: [], kids: [] }
      for (;;) {
        const d = peek()
        if (d === '.') { p++; n.cls.push(parts1()) }
        else if (d === '#') { p++; n.id = parts1() }
        else if (d === '@') { p++; n.ref = name() }
        else if (d === ':') {
          // the element's main attribute: a:#/about, img:pic.png, input:email
          p++
          const out = []
          let t = ''
          while (p < src.length) {
            const c = src[p]
            if (typeof c === 'object') { if (t) out.push(t), (t = ''); out.push(c); p++; continue }
            if (/[\s{}[\]>+^*()@]/.test(c)) break
            t += c
            p++
          }
          if (t) out.push(t)
          n.main = out
        }
        else if (d === '{') { p++; n.text = parts('}'); if (peek() !== '}') fail('a { without }'); p++ }
        else if (d === '[') {
          p++
          for (;;) {
            ws()
            if (peek() === ']') { p++; break }
            if (p >= src.length) fail('a [ without ]')
            const k = name()
            if (!k) fail('an attribute name')
            let v = []
            if (peek() === '=') {
              p++
              if (peek() === '"') { p++; v = parts('"'); p++ }
              else v = parts(']')
            }
            n.at.push([k, v])
          }
        } else if (typeof d === 'object') {
          // a slot right after an element shapes it
          n.mods.push(d.s)
          p++
        } else if (d === '*' && n.mul === undefined) {
          p++
          n.mul = count()
        } else break
      }
      if (!n.tag && !n.cls.length && !n.id && !n.text && !n.at.length && !n.mods.length && !n.ref && !n.main) fail('an element')
    }
    ws()
    if (peek() === '*' && n.mul === undefined) {
      p++
      n.mul = count()
    }
    return n
  }
  // *3, or *${items}
  function count() {
    const m = peek()
    if (typeof m === 'object') { p++; return { s: m.s } }
    let d = ''
    while (/\d/.test(peek())) d += src[p++]
    if (!d) fail('a count after *')
    return +d
  }
  // a class or id: letters and $ references
  function parts1() {
    const out = []
    let t = ''
    while (p < src.length) {
      const c = src[p]
      if (c === '$') {
        p++
        const path = []
        while (src[p] === '.' && typeof src[p + 1] === 'string' && /[A-Za-z_]/.test(src[p + 1])) { p++; path.push(name()) }
        if (t) out.push(t), (t = '')
        out.push({ it: path })
        continue
      }
      if (typeof c === 'string' && /[\w-]/.test(c)) { t += c; p++; continue }
      break
    }
    if (t) out.push(t)
    return out
  }
  // siblings with +, children with >, ^ climbs
  function seq() {
    const list = []
    for (;;) {
      const n = term()
      list.push(n)
      ws()
      const c = peek()
      if (c === '+') { p++; continue }
      if (c === '>') {
        p++
        const r = seq()
        n.kids = r.list
        if (r.up > 1) return { list, up: r.up - 1 }
        if (r.up === 1) continue
        return { list, up: 0 }
      }
      if (c === '^') {
        let u = 0
        while (peek() === '^') { p++; u++ }
        return { list, up: u }
      }
      return { list, up: 0 }
    }
  }
  const r = seq()
  ws()
  if (p < src.length) fail(`${typeof src[p] === 'string' ? src[p] : 'a slot'} out of place`)
  ast = r.list
  cache.set(strs, ast)
  return ast
}

// ---------- building ----------

const IMPLIED = { ul: 'li', ol: 'li', table: 'tr', tbody: 'tr', thead: 'tr', tfoot: 'tr', tr: 'td', select: 'option', datalist: 'option', optgroup: 'option', dl: 'dt', menu: 'li' }
const FORM = /^(INPUT|TEXTAREA|SELECT)$/
const MAIN = { a: 'href', link: 'href', area: 'href', base: 'href', img: 'src', script: 'src', iframe: 'src', video: 'src', audio: 'src', source: 'src', track: 'src', embed: 'src', input: 'type', button: 'type', form: 'action', label: 'for', option: 'value', meta: 'content', time: 'datetime', data: 'value', output: 'for', object: 'data', abbr: 'title', td: 'colspan', th: 'colspan', ol: 'start', textarea: 'placeholder', select: 'name' }
const PROPS = new Set(['value', 'checked', 'disabled', 'selected', 'hidden', 'textContent', 'innerHTML'])

const field = (it, path) => { let v = it; for (const k of path) v = v == null ? v : v[k]; return v }
const show = (v) => (v == null || v === false ? '' : String(v))

/** The text of parts: a string, or a function when a slot in it is live. */
function textOf(ps, ctx) {
  const vals = ps.map((x) => (typeof x === 'string' ? x : x.it ? field(ctx.it, x.it) : ctx.v[x.s]))
  if (!vals.some(isFn)) return vals.map(show).join('')
  return () => vals.map((v) => show(isFn(v) ? v() : v)).join('')
}

function setAttr(el, k, v) {
  if (k === 'class' && plain(v)) { for (const c in v) el.classList.toggle(c, !!(isFn(v[c]) ? v[c]() : v[c])); return }
  if (k === 'style' && plain(v)) { for (const s in v) el.style.setProperty(s.replace(/[A-Z]/g, (l) => '-' + l.toLowerCase()), show(isFn(v[s]) ? v[s]() : v[s])); return }
  if (PROPS.has(k) && k in el) { el[k] = k === 'value' || k === 'textContent' || k === 'innerHTML' ? show(v) : !!v; return }
  if (v == null || v === false) el.removeAttribute(k)
  else el.setAttribute(k, v === true ? '' : v)
}

function attr(el, k, v, ctx) {
  if (k.startsWith('on') && isFn(v) && !isSig(v)) {
    el.addEventListener(k.slice(2), (e) => v(e, ctx.has ? ctx.it : el, ctx.i))
    return
  }
  const live = isFn(v) || (plain(v) && Object.values(v).some(isFn))
  if (live) fx(() => setAttr(el, k, isFn(v) ? v() : v))
  else setAttr(el, k, v)
}

const EVENT = (el) => (el.tagName === 'FORM' ? 'submit' : el.tagName === 'SELECT' || (el.type === 'checkbox' || el.type === 'radio') ? 'change' : FORM.test(el.tagName) ? 'input' : 'click')

function shape(el, v, ctx) {
  if (isSig(v) && FORM.test(el.tagName)) return bind(el, v)
  if (isFn(v) && !isSig(v)) {
    const ev = EVENT(el)
    el.addEventListener(ev, (e) => {
      if (ev === 'submit') {
        // a form gives its values by name
        e.preventDefault()
        return v(e, Object.fromEntries(new FormData(el)), el)
      }
      v(e, ctx.has ? ctx.it : el, ctx.i)
    })
    return
  }
  if (plain(v)) { for (const k in v) attr(el, k, v[k], ctx); return }
  put(el, v, null)
}

// an input, select or textarea and a signal hold the same value
function bind(el, s) {
  const box = el.type === 'checkbox' || el.type === 'radio'
  fx(() => {
    const v = s()
    if (box) el.checked = !!v
    else if (el.value !== show(v)) el.value = show(v)
  })
  el.addEventListener(box || el.tagName === 'SELECT' ? 'change' : 'input', () => {
    if (box) s(el.checked)
    else s(typeof s.peek() === 'number' && el.value.trim() !== '' && !isNaN(el.value) ? +el.value : el.value)
  })
}

/** Puts a child value into parent before node at (or at the end): nodes, text, arrays, live regions. */
function put(parent, v, at) {
  if (v == null || v === false || v === true) return
  if (v instanceof Node) return parent.insertBefore(v, at)
  if (Array.isArray(v)) { for (const x of v) put(parent, x, at); return }
  if (isFn(v)) return region(parent, v, at)
  parent.insertBefore(document.createTextNode(String(v)), at)
}

// a part of the DOM made again when what its function read changes
function region(parent, f, at) {
  const start = document.createTextNode(''), end = document.createTextNode('')
  parent.insertBefore(start, at)
  parent.insertBefore(end, at)
  fx(() => {
    const v = f()
    const p = end.parentNode
    while (start.nextSibling && start.nextSibling !== end) start.nextSibling.remove()
    // a value that is only text keeps one text node
    put(p, v, end)
  })
}

function element(n, ctx, parentTag) {
  const tag = n.tag || IMPLIED[parentTag] || 'div'
  const el = document.createElement(tag)
  for (const c of n.cls) {
    const t = textOf(c, ctx)
    if (isFn(t)) { let old = ''; fx(() => { const v = t(); if (old) el.classList.remove(...old.split(' ').filter(Boolean)); if (v) el.classList.add(...v.split(' ').filter(Boolean)); old = v }) }
    else if (t) el.classList.add(t)
  }
  if (n.id) attr(el, 'id', textOf(n.id, ctx), ctx)
  for (const [k, ps] of n.at) {
    if (ps.length === 1 && ps[0].s !== undefined) attr(el, k, ctx.v[ps[0].s], ctx)
    else attr(el, k, ps.length ? textOf(ps, ctx) : true, ctx)
  }
  if (n.text) {
    const t = textOf(n.text, ctx)
    if (isFn(t)) { const tx = document.createTextNode(''); fx(() => { tx.data = t() }); el.append(tx) }
    else if (t) el.append(t)
  }
  if (n.main) {
    const ps = n.main
    attr(el, MAIN[tag] || 'title', ps.length === 1 && ps[0].s !== undefined ? ctx.v[ps[0].s] : textOf(ps, ctx), ctx)
  }
  if (n.ref) {
    ctx.refs[n.ref] = el
    // a form field's ref is its name too
    if (/^(INPUT|TEXTAREA|SELECT|BUTTON)$/.test(el.tagName) && !el.hasAttribute('name')) el.setAttribute('name', n.ref)
  }
  build(n.kids, el, ctx, tag)
  for (const s of n.mods) shape(el, ctx.v[s], ctx)
  return el
}

function one(n, parent, ctx, at) {
  if (n.t === 'e') parent.insertBefore(element(n, ctx, parent.tagName ? parent.tagName.toLowerCase() : ''), at)
  else if (n.t === 'g') { const f = document.createDocumentFragment(); build(n.kids, f, ctx, parent.tagName ? parent.tagName.toLowerCase() : ''); parent.insertBefore(f, at) }
  else put(parent, ctx.v[n.s], at)
}

function build(list, parent, ctx, parentTag) {
  for (const n of list) {
    if (n.mul === undefined) { one(n, parent, ctx, null); continue }
    const m = typeof n.mul === 'number' ? n.mul : ctx.v[n.mul.s]
    const each = (items, at) => items.forEach((it, i) => one(n, parent, { ...ctx, it, i, has: 1 }, at))
    if (typeof m === 'number') each(Array.from({ length: m }, (_, i) => i + 1), null)
    else if (isFn(m)) keyed(parent, n, ctx, m)
    else each(m || [], null)
  }
}

// *${signal}: a live list; an item that stays keeps its elements
function keyed(parent, n, ctx, s) {
  const start = document.createTextNode(''), end = document.createTextNode('')
  parent.append(start, end)
  let old = new Map() // item → { nodes, fx }
  fx(() => {
    const items = s() || []
    const p = end.parentNode
    const next = new Map()
    const owner = cur
    for (let i = 0; i < items.length; i++) {
      const it = items[i]
      let rec = old.get(it)
      if (rec && !next.has(it)) old.delete(it)
      else {
        // a fresh part: its effects belong to it, and go when it goes
        const f = document.createDocumentFragment()
        const e = { deps: new Set(), kids: [], dead: 0 }
        const prev = cur
        cur = e
        try { one(n, f, { ...ctx, it, i, has: 1 }, null) } finally { cur = prev }
        rec = { nodes: [...f.childNodes], e }
      }
      next.set(next.has(it) ? Symbol() : it, rec)
      for (const nd of rec.nodes) p.insertBefore(nd, end)
    }
    for (const rec of old.values()) { for (const nd of rec.nodes) nd.remove(); dispose(rec.e) }
    old = next
    void owner
  })
}

function tpl(strs, v) {
  const ast = read(strs)
  const ctx = { v, refs: {}, it: undefined, i: undefined, has: 0 }
  const f = document.createDocumentFragment()
  build(ast, f, ctx, '')
  const out = f.childNodes.length === 1 ? f.firstChild : f
  out.$ = ctx.refs
  return out
}

// ---------- $ ----------

export const $ = (s, ...v) => (Array.isArray(s) && s.raw ? tpl(s, v) : sig(s))

$.fx = fx
/** A computed signal: it follows what f reads. */
$.c = (f) => { const s = sig(undefined); fx(() => s(f())); const r = () => s(); r.$s = 1; r.peek = s.peek; return r }
/** A style sheet; a function or signal in it keeps it live. */
$.css = (strs, ...v) => {
  const el = document.createElement('style')
  document.head.append(el)
  fx(() => { el.textContent = strs.reduce((a, s, i) => a + s + (i < v.length ? show(isFn(v[i]) ? v[i]() : v[i]) : ''), '') })
  return el
}
/** A signal kept in localStorage under name. */
$.keep = (name, s) => {
  try { const j = localStorage.getItem('$:' + name); if (j !== null) s(JSON.parse(j)) } catch {}
  fx(() => { const v = s(); try { localStorage.setItem('$:' + name, JSON.stringify(v)) } catch {} })
  return s
}
/** The address after #, as a signal. */
$.hash = sig(typeof location !== 'undefined' ? decodeURIComponent(location.hash.slice(1)) : '')
if (typeof addEventListener !== 'undefined') addEventListener('hashchange', () => $.hash(decodeURIComponent(location.hash.slice(1))))
/** Appends nodes to the element sel names (or the body). */
$.at = (sel, ...nodes) => { const el = typeof sel === 'string' ? document.querySelector(sel) : sel || document.body; el.append(...nodes); return el }
$.q = (sel) => document.querySelector(sel)
$.qa = (sel) => [...document.querySelectorAll(sel)]

if (typeof window !== 'undefined' && !window.$) window.$ = $
export default $
