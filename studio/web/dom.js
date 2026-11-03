// s in the page: element words, modifiers, routes, kept state. A program describes the page;
// running it builds the elements. While an element's brace runs, the names it reads are
// recorded; when an event or a timer writes one of them, that brace runs again.
import { text, valOf, num, Reads } from './s.js'

export const VOID = new Set(['area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'source', 'track', 'wbr'])
export const TAGS = new Set(('a abbr address article aside audio b bdi bdo blockquote body br button canvas caption cite code col colgroup data datalist dd del details dfn dialog div dl dt em fieldset figcaption figure footer form h1 h2 h3 h4 h5 h6 header hgroup hr i iframe img input ins kbd label legend li link main map mark menu meta meter nav noscript object ol optgroup option output p picture pre progress q rp rt ruby s samp section select slot small source span strong sub summary sup table tbody td template textarea tfoot th thead time tr track u ul var video wbr').split(' '))
export const EVENTS = new Set('click dblclick input change submit keydown keyup focus blur mouseenter mouseleave mousedown mouseup pointerdown pointerup contextmenu wheel scroll load toggle'.split(' '))

const kebab = (s) => s.replace(/[A-Z]/g, (l) => '-' + l.toLowerCase())

/** `div.card.other#id` or `<div.card#id`: the tag, its classes (camelCase becomes kebab-case)
 *  and its id. */
export function elementSpec(w) {
  const m = /^<?([a-z][a-z0-9]*)((?:[.#][A-Za-z_][\w-]*)*)$/.exec(w)
  if (!m || !TAGS.has(m[1])) return null
  const classes = [], ids = []
  for (const p of m[2].match(/[.#][A-Za-z_][\w-]*/g) || []) (p[0] === '.' ? classes : ids).push(kebab(p.slice(1)))
  return { tag: m[1], classes, id: ids[ids.length - 1] || null }
}
export const attrWord = (w) => /^[a-z][\w-]*=$/.test(w)
export const eventWord = (w) => w.length > 2 && w.startsWith('on') && EVENTS.has(w.slice(2))
export const selWord = (w) => /^\$[.#a-zA-Z[:*]/.test(w)
const component = (rt, w) => /^[A-Z]/.test(w) && rt.repl.has(w)

/** A child brace runs as code when it holds a brace, a quote, a bead mark, a selector, an
 *  attribute, an event, a component, a call of a defined script, ~ &&& ?) >< ##, an element
 *  word with an angle bracket, a class or an id, or when its last word is an element word;
 *  otherwise it is text. */
export function isCode(rt, t) {
  if (/[{}'⟨⟪⨝]/.test(t)) return true
  const ws = t.split(/\s+/).filter(Boolean)
  const last = ws[ws.length - 1]
  if (ws.length > 1 && last.length > 1 && elementSpec(last)) return true
  for (const w of ws) {
    if ((w[0] === '<' || /[.#]/.test(w)) && elementSpec(w)) return true
    if (selWord(w) || attrWord(w) || eventWord(w) || component(rt, w)) return true
    if (w === '&&&' || w === '~' || w === '?)' || w === '><' || w === '##') return true
    if (w.length > 1 && w.endsWith('.') && rt.scripts[rt.it.sym(w.replace(/\.+$/, ''))]) return true
  }
  return false
}

const fresh = () => ({ classes: [], attrs: [], events: [], at: null, routes: false })
const evalParts = (rt, parts) => parts.map((p) => (p[0] === 'T' ? p[1] : rt.interp(p[1]))).join('')

export function attachDom(rt, opts = {}) {
  const doc = opts.document || document
  const win = doc.defaultView
  const mount = opts.mount || doc.body
  const store = opts.storage === undefined ? safeStorage(win) : opts.storage
  const D = {
    mount,
    parents: [mount],
    pending: fresh(),
    regions: [],  // { el, code, parts, reads }: a brace that runs again when a name it read changes
    attrs: [],    // { el, name, parts, reads }: an attribute made from names
    twoWay: [],   // { el, k }: an input that holds a name
    events: [],   // { el, ev, code }
    timers: [],   // { id, code, ms, repeat }
    pages: new Map(),
    routers: [],
    kept: new Set(),
    rules: new Set(),
    all: false,
    busy: false,
    onError: opts.onError || ((e) => console.error('s:', e)),
    budget: opts.budget || 5_000_000,
  }
  rt.dom = D
  rt.tracking = true
  const top = () => D.parents[D.parents.length - 1]
  const sheet = () => {
    let s = doc.getElementById('s-sheet')
    if (!s) { s = doc.createElement('style'); s.id = 's-sheet'; doc.head.append(s) }
    return s
  }

  // the line before an element, and what is left at the end of a block, become text
  function flushText(parent, before) {
    if (!rt.args.length) return
    const t = rt.args.map(text).join(' ')
    rt.args = []
    if (!t) return
    const lead = !before && parent.childNodes.length ? ' ' : ''
    parent.append(doc.createTextNode(lead + t + (before ? ' ' : '')))
  }

  /** A value the line got from a brace with names in it, just now: its parts and the names. */
  function interpolated() {
    const li = rt.lastInterp
    if (!li || li.at !== rt.args.length || rt.args[rt.args.length - 1] !== li.v) return null
    rt.lastInterp = null
    if (rt.reads instanceof Reads) for (const k of li.reads) rt.reads.sub(k)
    return li
  }

  // runs a block of children inside el: its own line, el the parent, leftover text at the end;
  // the names the block reads make el a region. c: whether the block is code (codeOf).
  function runChildren(el, child, parts, pre, c) {
    if (!c) {
      if (child) el.append(doc.createTextNode(child))
      if (pre && pre.size) D.regions.push({ el, code: parts ? null : child, parts, reads: new Set(pre), c })
      return
    }
    const saved = rt.args, outer = rt.reads
    rt.args = []
    rt.reads = new Reads()
    if (pre) for (const k of pre) rt.reads.add(k)
    D.parents.push(el)
    rt.fs.push(rt.hostFrame(() => {
      flushText(el, false)
      D.parents.pop()
      rt.args = saved
      const reads = rt.reads
      rt.reads = outer
      if (reads.size) D.regions.push({ el, code: parts ? null : child, parts, reads: new Set(reads.keys()), c })
    }))
    rt.runSrc(child)
  }

  /** Whether a block is code: what a name holds never decides, a brace with names is code when its own text is. */
  function codeOf(child, parts) { return isCode(rt, parts ? parts.map((p) => (p[0] === 'T' ? p[1] : ' x ')).join('') : child) }
  const kids = (el, child, parts = null, pre = null) => runChildren(el, child, parts, pre, codeOf(child, parts))

  function apply(el, p) {
    // shake: uses refreshAttrs
    if (p.classes.length) el.classList.add(...p.classes)
    for (const [k, v, li] of p.attrs) {
      const two = li && (k === 'value' || k === 'checked') && /^(INPUT|SELECT|TEXTAREA)$/.test(el.tagName) && li.parts.length === 1 && li.parts[0][0] === 'V'
      if (two) { twoWay(el, li.parts[0][1]); continue }
      setAttr(el, k, v)
      if (li && li.reads.size) D.attrs.push({ el, name: k, parts: li.parts, reads: new Set(li.reads) })
    }
    for (const [ev, code] of p.events) listen(el, ev, code)
  }
  function setAttr(el, k, v) {
    if (k === 'class') el.classList.add(...v.split(/\s+/).filter(Boolean))
    else if (k === 'style') el.style.cssText += ';' + v
    else if (k === 'value' && 'value' in el) { el.value = v; el.setAttribute('value', v) }
    else if (k === 'checked' && 'checked' in el) { el.checked = v !== '' && v !== '0'; if (el.checked) el.setAttribute('checked', '') }
    else el.setAttribute(k, v)
  }
  function listen(el, ev, code) {
    D.events.push({ el, ev, code })
    el.addEventListener(ev, (e) => {
      if (ev === 'submit') e.preventDefault()
      handler(code, e, el)
    })
  }

  function element(spec) {
    const p = D.pending
    D.pending = fresh()
    let child = null, li = null
    if (!VOID.has(spec.tag)) {
      if (!rt.args.length) rt.fail(`${spec.tag}: needs its children, a brace (or {} for none)`)
      li = interpolated()
      child = text(rt.args.pop())
    }
    const parent = p.at ? query(p.at) : top()
    if (!p.at) flushText(parent, true)
    const el = doc.createElement(spec.tag)
    if (spec.classes.length) el.classList.add(...spec.classes)
    if (spec.id) el.id = spec.id
    apply(el, p)
    // elements written one after another sit apart, as they do in HTML source
    if (parent.lastChild && parent.lastChild.nodeType === 1) parent.append(doc.createTextNode(' '))
    parent.append(el)
    if (p.routes) {
      D.routers.push(el)
      const page = currentPage()
      if (page !== null) kids(el, page)
      return
    }
    if (child !== null) kids(el, child, li ? li.parts : null, li ? li.reads : null)
  }

  function query(sel) {
    const e = doc.querySelector(sel)
    if (!e) rt.fail(`no element matches ${sel}`)
    return e
  }

  // shake: host
  rt.hostWord = (rt2, w, fromValue) => {
    if (w[0] === '<' || !fromValue) {
      const spec = elementSpec(w)
      if (spec) { element(spec); return true }
    }
    if (attrWord(w)) {
      const li = interpolated()
      const v = text(rt.pop(w))
      D.pending.attrs.push([w.slice(0, -1), v, li])
      return true
    }
    if (eventWord(w)) {
      const code = text(rt.pop(w))
      D.pending.events.push([w.slice(2), code])
      return true
    }
    if (selWord(w)) {
      const code = text(rt.pop(w))
      kids(query(w.slice(1)), code)
      return true
    }
    return false
  }

  // A library that names a word like an element (std's div, sub, map) keeps it in its own
  // bodies; everywhere else the word makes the element.
  // shake: host
  rt.preWord = (w) => {
    if (!TAGS.has(w)) return false
    const m = rt.visible(w)
    if (!m || !m.lib || rt.inLib()) return false
    element(elementSpec(w))
    return true
  }

  // ---------- events, timers, two-way inputs ----------

  function setEventNames(e, el) {
    const t = (e && e.target) || el
    const set = (n, v) => { rt.mem[rt.it.sym(n)] = v }
    set('e.value', valOf(t && t.value != null ? String(t.value) : ''))
    set('e.checked', t && t.checked ? 1 : 0)
    set('e.key', e && e.key ? e.key : '')
    set('e.id', t && t.id ? t.id : '')
    set('e.text', valOf(t && t.textContent != null ? t.textContent.trim() : ''))
  }

  /** Runs code for an event or a timer; the names it writes decide what runs again. */
  function act(code, label) {
    if (D.busy) { rt.runSrc(code); return }
    D.busy = true
    rt.args = []
    D.parents = [mount]
    const outer = rt.writes
    rt.writes = new Set()
    const st = rt.runText(code, D.budget)
    const wrote = rt.writes
    rt.writes = outer
    D.busy = false
    report(st, label)
    // shake: weak save
    if (D.kept.size) save()
    refresh(wrote)
  }
  function report(st, label) {
    if (st === 'ran') D.onError({ msg: `${label}: stopped after ${D.budget} ops`, code: rt.siteCode, pc: rt.site[1] })
    else if (st && st.error) D.onError({ msg: st.error, code: rt.siteCode, pc: rt.site[1] })
  }
  function handler(code, e, el) {
    // shake: weak setEventNames
    if (typeof setEventNames === 'function') setEventNames(e, el)
    act(code, 'on' + (e ? e.type : ''))
  }

  function nameText(k) {
    if (rt.mem[k] !== undefined) return text(rt.mem[k])
    if (rt.nums[k] !== undefined) return text(rt.nums[k])
    return ''
  }
  function showName(el, k) {
    const v = nameText(k)
    if (el.type === 'checkbox') { el.checked = v !== '' && num(v) !== 0; el.toggleAttribute('checked', el.checked) }
    else if (el.value !== v) { el.value = v; el.setAttribute('value', v) }
  }
  function twoWay(el, k) {
    // shake: uses refreshInputs
    el.dataset.bind = rt.name(k)
    D.twoWay.push({ el, k })
    showName(el, k)
    el.addEventListener(el.tagName === 'SELECT' || el.type === 'checkbox' ? 'change' : 'input', () => {
      if (el.type === 'checkbox') { rt.nums[k] = el.checked ? 1 : 0; rt.mem[k] = undefined }
      else rt.mem[k] = valOf(el.value)
      // shake: weak save
      if (D.kept.size) save()
      refresh(new Set([k]))
    })
  }

  // ---------- running again ----------

  const meets = (reads, dirty) => { for (const k of reads) if (dirty.has(k)) return true; return false }

  /** Runs again every region and attribute that read a name in dirty (or all of them). */
  function refresh(dirty) {
    // shake: weak currentPage kids refreshAttrs refreshInputs
    if (D.all) { D.all = false; dirty = 'all' }
    if (D.busy) { if (dirty === 'all') D.all = true; return }
    if (dirty !== 'all' && (!dirty || !dirty.size)) return
    const hit = (r) => dirty === 'all' || meets(r.reads, dirty)
    const a = doc.activeElement
    const key = a && a !== doc.body && (a.id ? '#' + a.id : a.dataset && a.dataset.bind ? `[data-bind="${a.dataset.bind}"]` : null)
    const sel = a && 'selectionStart' in a ? (() => { try { return [a.selectionStart, a.selectionEnd] } catch { return null } })() : null
    D.busy = true
    const live = (x) => x.el.isConnected
    D.regions = D.regions.filter(live)
    let todo = D.regions.filter(hit)
    if (dirty === 'all') for (const r of D.routers) if (r.isConnected) todo.push({ el: r, router: true })
    // the outermost of nested regions runs; what is inside it is made again
    todo = todo.filter((r) => !todo.some((o) => o !== r && o.el !== r.el && o.el.contains(r.el)))
    for (const R of todo) {
      if (!R.el.isConnected) continue
      const inside = (x) => x.el !== R.el && R.el.contains(x.el)
      D.regions = D.regions.filter((x) => x !== R && !(R.router && x.el === R.el) && !inside(x))
      D.attrs = D.attrs.filter((x) => !inside(x))
      D.twoWay = D.twoWay.filter((x) => !inside(x))
      D.events = D.events.filter((x) => !inside(x))
      D.routers = D.routers.filter((x) => x === R.el || !R.el.contains(x))
      R.el.replaceChildren()
      rt.args = []
      D.parents = [mount]
      if (R.router) {
        const page = currentPage()
        if (page !== null) kids(R.el, page)
      } else if (R.parts) {
        const mine = new Set(), outer = rt.reads
        rt.reads = mine
        const t = evalParts(rt, R.parts)
        rt.reads = outer
        runChildren(R.el, t, R.parts, mine, R.c)
      } else runChildren(R.el, R.code, null, null, R.c)
      report(rt.run(D.budget), 'update')
      rt.args = []
    }
    if (D.attrs.length) refreshAttrs(hit, live)
    if (D.twoWay.length) refreshInputs(dirty, live)
    D.parents = [mount]
    D.busy = false
    if (key) {
      const b = doc.querySelector(key)
      if (b && b !== doc.activeElement) {
        b.focus()
        if (sel && 'setSelectionRange' in b) try { b.setSelectionRange(sel[0], sel[1]) } catch {}
      }
    }
    if (D.all) refresh('all')
  }

  /** Attributes made from names, again. */
  function refreshAttrs(hit, live) {
    D.attrs = D.attrs.filter(live)
    for (const A of D.attrs) {
      if (!hit(A)) continue
      const mine = new Set(), outer = rt.reads
      rt.reads = mine
      const v = evalParts(rt, A.parts)
      rt.reads = outer
      A.reads = mine
      if (A.name === 'class' || A.name === 'style') A.el.setAttribute(A.name, v)
      else setAttr(A.el, A.name, v)
    }
  }
  /** Inputs that hold names, again, except the one being typed in. */
  function refreshInputs(dirty, live) {
    D.twoWay = D.twoWay.filter(live)
    for (const b of D.twoWay) {
      if ((dirty === 'all' || dirty.has(b.k)) && !(b.el === doc.activeElement && b.el.type !== 'checkbox')) showName(b.el, b.k)
    }
  }

  // ---------- pages ----------

  function currentPage() {
    const h = decodeURIComponent((win.location.hash || '').replace(/^#\/?/, ''))
    if (D.pages.has(h)) return D.pages.get(h)
    const first = D.pages.values().next()
    return first.done ? null : first.value
  }
  function onHash() {
    if (!D.routers.length) return
    D.busy = true
    const rs = D.routers.filter((r) => r.isConnected)
    D.busy = false
    for (const r of rs) {
      D.busy = true
      D.regions = D.regions.filter((x) => !r.contains(x.el))
      D.attrs = D.attrs.filter((x) => !r.contains(x.el))
      D.twoWay = D.twoWay.filter((x) => !r.contains(x.el))
      D.events = D.events.filter((x) => !r.contains(x.el))
      r.replaceChildren()
      rt.args = []
      D.parents = [mount]
      const page = currentPage()
      if (page !== null) kids(r, page)
      report(rt.run(D.budget), 'page')
      D.busy = false
    }
  }
  win.addEventListener('hashchange', onHash)

  // ---------- kept state ----------

  function restore(name) {
    if (!store) return
    let j
    try { j = JSON.parse(store.getItem('s:' + name) || 'null') } catch { j = null }
    if (!j) return
    const k = rt.it.sym(name)
    if ('mem' in j) rt.mem[k] = j.mem
    if ('num' in j) rt.nums[k] = j.num
    if ('list' in j) rt.lists[k] = j.list
    return k
  }
  function save() {
    if (!store) return
    for (const name of D.kept) {
      const k = rt.it.sym(name)
      const j = {}
      if (rt.mem[k] !== undefined) j.mem = typeof rt.mem[k] === 'object' ? rt.mem[k].w : rt.mem[k]
      if (rt.nums[k] !== undefined) j.num = rt.nums[k]
      if (rt.lists[k] !== undefined) j.list = rt.lists[k].map((v) => (typeof v === 'object' ? v.w : v))
      try { store.setItem('s:' + name, JSON.stringify(j)) } catch {}
    }
  }

  // ---------- the words ----------

  const word = (name, f) => rt.native(name, f)
  word('refresh', () => { D.all = true; if (!D.busy) refresh('all') })
  word('at', (r) => { D.pending.at = text(r.pop('at')) })
  word('routes', () => { D.pending.routes = true })
  word('page', (r) => {
    const name = text(r.pop('page'))
    const code = text(r.pop('page'))
    D.pages.set(name, code)
  })
  word('css', (r) => {
    const sel = text(r.pop('css'))
    const body = text(r.pop('css'))
    if (D.rules.has(sel)) return
    D.rules.add(sel)
    sheet().append(doc.createTextNode(`\n${sel} {${body}}`))
  })
  word('sheet', (r) => { sheet().append(doc.createTextNode('\n' + text(r.pop('sheet')))) })
  word('html', (r) => {
    const h = text(r.pop('html'))
    const parent = top()
    flushText(parent, true)
    parent.insertAdjacentHTML('beforeend', h)
  })
  word('each', (r) => {
    const code = text(r.pop('each'))
    const name = text(r.pop('each'))
    const k = r.it.sym(name)
    r.read(k)
    const items = (r.lists[k] || []).slice()
    const it = r.it.sym('it'), ix = r.it.sym('i')
    for (let n = items.length - 1; n >= 0; n--) {
      r.runSrc(code)
      r.fs.push(r.hostFrame(() => { r.mem[it] = items[n]; r.nums[ix] = n; r.mem[ix] = undefined }))
    }
  })
  word('keep', (r) => {
    // shake: uses save
    const name = text(r.pop('keep'))
    D.kept.add(name)
    restore(name)
  })
  const timer = (repeat) => (r) => {
    const ms = Math.max(16, num(r.pop(repeat ? 'every' : 'after')))
    const code = text(r.pop(repeat ? 'every' : 'after'))
    startTimer(code, ms, repeat)
  }
  function startTimer(code, ms, repeat) {
    const f = () => act(code, repeat ? 'every' : 'after')
    D.timers.push({ id: repeat ? win.setInterval(f, ms) : win.setTimeout(f, ms), code, ms, repeat })
  }
  word('every', timer(true))
  word('after', timer(false))
  word('title', (r) => { doc.title = text(r.pop('title')) })
  word('now', (r) => { r.args.push(Date.now()) })
  word('time', (r) => { r.args.push(new Date().toLocaleTimeString()) })
  word('date', (r) => { r.args.push(new Date().toLocaleDateString()) })

  const api = {
    D,
    run(src) {
      D.busy = true
      D.parents = [mount]
      rt.unitIds.set(src, 0)
      const st = rt.runText(src, D.budget)
      D.busy = false
      report(st, 'the program')
      save()
      if (D.all) refresh('all')
      return st
    },
    refresh,
    stop() {
      for (const t of D.timers) { win.clearInterval(t.id); win.clearTimeout(t.id) }
      D.timers = []
      win.removeEventListener('hashchange', onHash)
    },
    // used by a compiled page: the records come back attached to elements already there
    resume(R) {
      const at = (i) => doc.querySelector(`[data-s="${i}"]`)
      for (const k of ['events', 'regions', 'attrs', 'twoWay', 'pages', 'rules', 'kept', 'timers', 'routers']) if (R[k].length) RESUME[k](R[k], at)
    },
  }
  // what a compiled page brings back, one kind of record each (a page keeps the kinds it has)
  const RESUME = {
    events(l, at) { for (const [i, ev, code] of l) listen(at(i), ev, code) },
    regions(l, at) { for (const [i, code, parts, reads, c] of l) D.regions.push({ el: at(i), code, parts, reads: new Set(reads), c: !!c }) },
    attrs(l, at) {
      // shake: uses refreshAttrs
      for (const [i, name, parts, reads] of l) D.attrs.push({ el: at(i), name, parts, reads: new Set(reads) }) },
    twoWay(l, at) { for (const [i, k] of l) twoWay(at(i), k) },
    pages(l) { for (const [n, c] of l) D.pages.set(n, c) },
    rules(l) { for (const s of l) D.rules.add(s) },
    kept(l) {
      // shake: uses save
      const back = new Set()
      for (const n of l) { D.kept.add(n); const k = restore(n); if (k !== undefined) back.add(k) }
      if (back.size) refresh(back)
    },
    timers(l) { for (const [code, ms, repeat] of l) startTimer(code, ms, repeat) },
    routers(l, at) {
      for (const i of l) D.routers.push(at(i))
      if ((win.location.hash || '').length > 1) onHash()
    },
  }
  return api
}

function safeStorage(win) {
  try { const s = win.localStorage; s.getItem('x'); return s } catch { return null }
}
