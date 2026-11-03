// The editor for s: the files of s3-studio's folder in tabs, the zone and lane editor with
// words coloured as the machine reads them, runs on the machine with what each line printed
// beside it, builds to Rust, definitions and uses, completion, the palette, find and replace.
import { Editor } from '../web/ed.js'
import { encode } from '../web/s5.js'

const $ = (s) => document.querySelector(s)
const out = $('#out'), foot = $('#where')
const store = {
  get(k, d) { try { const v = localStorage.getItem('s.' + k); return v == null ? d : JSON.parse(v) } catch { return d } },
  set(k, v) { try { localStorage.setItem('s.' + k, JSON.stringify(v)) } catch {} },
}
async function api(cmd, o = {}) {
  const r = await fetch('api', { method: 'POST', body: JSON.stringify({ cmd, ...o }) })
  const j = await r.json()
  if (j && j.error && typeof j.error === 'string') throw new Error(j.error)
  return j
}
const utf8 = (s) => new TextEncoder().encode(s).length
const kb = (n) => (n < 1024 ? n + ' B' : (n / 1024).toFixed(1) + ' KB')
/** A character offset as the machine counts (code points) as an index into the JS string. */
const jsIndex = (s, cp) => { let i = 0, n = 0; for (const ch of s) { if (n === cp) return i; i += ch.length; n++ } return i }
const lineOf = (s, i) => s.slice(0, i).split('\n').length
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

// ---------- what a word is ----------

const SUFFIX = '?!~$`#;:.@'
const kinds = new Map()       // word → the class the machine reads it as
const pending = new Set()
let kindTimer = 0
const CLASS = { num: 'num', cmd: 'cmd', esc: 'esc', lib: 'lib', look: 'look', pop: 'pop', def: 'def', set: 'set', numset: 'numset', call: 'call', geo: 'geo' }
function wordKind(w) {
  if (/^-?\d+(\.\d+)?([eE][-+]?\d+)?$/.test(w)) return 'num'
  if (/\.s\.$/.test(w)) return 'load'
  if (kinds.has(w)) return kinds.get(w)
  pending.add(w)
  if (!kindTimer) kindTimer = setTimeout(askKinds, 40)
  return ''
}
async function askKinds() {
  kindTimer = 0
  const ws = [...pending].filter((w) => !kinds.has(w))
  pending.clear()
  if (!ws.length) return
  let any = false
  try {
    const r = await api('lex', { text: ws.join(' ') })
    for (const t of r.tokens || []) { const k = CLASS[t.kind] || ''; kinds.set(t.w, k); if (k) any = true }
  } catch {}
  for (const w of ws) if (!kinds.has(w)) kinds.set(w, '')
  if (any) redraw()
}
const LOOPS = new Set(['times', 'while', 'until', 'map', 'filter', 'fold', 'foldr', 'scan', 'reduce', 'lanewise', 'each', 'every'])
const BRANCH = new Set(['?)', 'if', 'when', 'unless', 'test'])
function zoneKind(w) {
  if (!w) return 'plain'
  if (w === '//') return 'note'
  if (w === 'try') return 'try'
  if (w === '><' || (w.length > 1 && w.endsWith('$') && w[0] !== '\\')) return 'def'
  if (BRANCH.has(w) || /[?!]$/.test(w) && w.length > 1 && BRANCH.has(w.slice(0, -1))) return 'branch'
  if (LOOPS.has(w)) return 'loop'
  return 'plain'
}

// the commands, std words and sigils, from the server
const docs = new Map()        // word → { sig, doc, kind, file }
const caps = new Map()        // sigil → { name, doc }
let groups = []
async function loadDocs() {
  try {
    const p = await api('palette')
    groups = (p.groups || []).filter((g) => g.name !== 'graph')
    for (const g of groups) for (const it of g.items) if (!docs.has(it.w)) docs.set(it.w, { sig: it.sig || '', doc: it.doc || '', kind: it.kind, file: it.kind === 'lib' ? g.name : null })
    for (const c of p.caps || []) caps.set(c.sigil, c)
  } catch {}
}

/** A word without its sigils: the name it reads or defines. */
function bare(w) {
  let s = w
  if (s.startsWith("'") && s.endsWith("'") && s.length > 1) return s.slice(1, -1)
  if (s.startsWith('\\') && s.length > 1) s = s.slice(1)
  while (s.length > 1 && SUFFIX.includes(s[s.length - 1])) s = s.slice(0, -1)
  return s
}
/** Tokens with the depth of braces they stand in; a close token carries what follows the brace. */
function tokenize(text) {
  const toks = []
  const n = text.length
  let i = 0, depth = 0
  const stop = (c) => /\s/.test(c) || c === '{' || c === '}'
  while (i < n) {
    const c = text[i]
    if (/\s/.test(c)) { i++; continue }
    const from = i
    if (c === '{') { toks.push({ from, to: i + 1, w: '{', kind: 'open', depth }); depth++; i++; continue }
    if (c === '}') {
      depth = Math.max(0, depth - 1)
      let j = i + 1
      while (j < n && !stop(text[j])) j++
      toks.push({ from, to: j, w: text.slice(from, j), kind: 'close', depth })
      i = j
      continue
    }
    if (c === "'") {
      let j = i + 1
      while (j < n && text[j] !== "'" && text[j] !== '\n') j++
      j = Math.min(n, j + 1)
      toks.push({ from, to: j, w: text.slice(from, j), kind: 'quote', depth })
      i = j
      continue
    }
    let j = i
    while (j < n && !stop(text[j])) j++
    toks.push({ from, to: j, w: text.slice(from, j), kind: 'word', depth })
    i = j
  }
  return toks
}
/** Definitions in a text: {…} 'key' ><, {…} key ><, {…} name$ — name → [{ from, to }]. */
function defsOf(text) {
  const defs = new Map()
  const ts = tokenize(text)
  const add = (name, t) => { if (!name) return; if (!defs.has(name)) defs.set(name, []); defs.get(name).push({ from: t.from, to: t.to }) }
  ts.forEach((t, i) => {
    if (t.depth !== 0) return
    if (t.w === '><' && i >= 2 && ts[i - 2].kind === 'close' && ts[i - 2].depth === 0) {
      const k = ts[i - 1]
      add(bare(k.w), k.kind === 'quote' ? { from: k.from + 1, to: k.to - 1 } : k)
    }
    if (t.kind === 'word' && t.w.endsWith('$') && !t.w.endsWith('\\$') && t.w.length > 1 && i >= 1 && ts[i - 1].kind === 'close') add(bare(t.w), t)
  })
  return defs
}
/** The paragraphs of a text that define something or load a file: the console's prelude. */
function prelude(text) {
  return text.split(/\n[ \t]*\n/).filter((p) => defsOf(p).size || tokenize(p).some((t) => t.depth === 0 && /\.s\.$/.test(t.w))).join('\n\n')
}

// ---------- the editor ----------

let ready = false // the hooks below wait until everything they use is defined
const ed = new Editor($('#ed'), {
  word: wordKind, zone: zoneKind, prose: (t, w) => w === '//',
  numbers: true,
  onChange: () => ready && changed(),
  onCaret: () => ready && caretInfo(),
  onKey: (e) => compKey(e) || edKey(e),
  onHover: (x, cx, cy) => hover(x, cx, cy),
  onGo: (x) => goDef(x),
})
window.ed = ed
function redraw() {
  const { a, b } = ed, top = ed.scroll.scrollTop
  ed.render()
  ed.a = a; ed.b = b
  ed.scroll.scrollTop = top
  ed.paint()
}

// ---------- tabs ----------

let tabs = [], cur = -1
const tab = () => tabs[cur] || null
const isLib = (p) => /^std\//.test(p)
const dirtyTab = (t) => !!t && !t.lib && (t === tab() ? ed.value : t.text) !== t.saved
function stash() {
  const t = tab()
  if (!t) return
  t.text = ed.value; t.a = ed.a; t.b = ed.b; t.undos = ed.undos; t.redos = ed.redos; t.top = ed.scroll.scrollTop
  t.args = $('#args').value; t.stdin = $('#stdin').value
  if (!t.lib) store.set('input:' + t.path, { args: t.args, stdin: t.stdin })
}
function show(i) {
  if (i === cur && tab()) return
  stash()
  cur = i
  const t = tab()
  clearRun()
  if (!t) { ed.value = ''; ed.setReadOnly(true); renderTabs(); showList(); return }
  ed.value = t.text
  ed.undos = t.undos || []; ed.redos = t.redos || []
  ed.setReadOnly(t.lib)
  ed.a = t.a ?? 0; ed.b = t.b ?? 0
  ed.scroll.scrollTop = t.top || 0
  ed.focus(); ed.paint()
  $('#args').value = t.args || ''; $('#stdin').value = t.stdin || ''
  if (find.open) findHits()
  store.set('cur', t.path)
  renderTabs(); showList(); measure()
}
async function openFile(p, at = null) {
  let i = tabs.findIndex((t) => t.path === p)
  if (i < 0) {
    try {
      const lib = isLib(p)
      const j = lib ? await api('lib', { path: p }) : await api('read', { path: p })
      if (lib && j.text == null) throw new Error(p + ': not in std')
      const inp = lib ? {} : store.get('input:' + p, {})
      tabs.push({ path: p, text: j.text, saved: j.text, disk: j.text, lib, a: 0, b: store.get('caret:' + p, 0), args: inp.args || '', stdin: inp.stdin || '' })
      i = tabs.length - 1
      keepTabs()
    } catch (e) { fail(e.message); return false }
  }
  show(i)
  if (at !== null) { ed.select(at[0], at[1]); ed.focus() }
  return true
}
function closeTab(i) {
  const t = tabs[i]
  if (!t) return
  if (dirtyTab(t) && !confirm(`${t.path} has unsaved changes. Close it and drop them?`)) return
  if (i === cur) { cur = -1; tabs.splice(i, 1); show(Math.min(i, tabs.length - 1)) }
  else { tabs.splice(i, 1); if (i < cur) cur--; renderTabs() }
  keepTabs()
}
function keepTabs() { store.set('tabs', tabs.filter((t) => !t.lib).map((t) => t.path)) }
function renderTabs() {
  const el = $('#tabs')
  el.replaceChildren()
  tabs.forEach((t, i) => {
    const d = document.createElement('div')
    d.className = 'tab' + (i === cur ? ' on' : '') + (dirtyTab(t) ? ' dirty' : '') + (t.lib ? ' lib' : '')
    d.title = t.path + (t.lib ? ' (std, read-only)' : '')
    const nm = document.createElement('span')
    nm.className = 'nm'
    nm.textContent = t.path.split('/').pop()
    const x = document.createElement('button')
    x.className = 'x'; x.textContent = '×'; x.title = 'Close (Alt+W)'
    x.onclick = (e) => { e.stopPropagation(); closeTab(i) }
    d.append(nm, x)
    d.onclick = () => show(i)
    d.onauxclick = (e) => { if (e.button === 1) closeTab(i) }
    el.append(d)
  })
  const t = tab()
  document.title = t ? (dirtyTab(t) ? '• ' : '') + t.path : 's'
}
async function save() {
  const t = tab()
  if (!t) { openPicker(); fail('a name for the file: type name.s and Enter'); return }
  if (t.lib) { fail(t.path + ' is std, read-only'); return }
  const text = ed.value
  try {
    await api('write', { path: t.path, text })
    t.saved = text; t.disk = text
    renderTabs()
    note('saved ' + t.path)
  } catch (e) { fail(e.message) }
}
async function newFile(p) {
  try { await api('write', { path: p, text: '' }); await listFiles(); await openFile(p) } catch (e) { fail(e.message) }
}
// a file changed on disk comes back into a tab that has nothing unsaved
addEventListener('focus', async () => {
  for (const t of tabs) {
    if (t.lib) continue
    try {
      const j = await api('read', { path: t.path })
      if (j.text === t.disk) continue
      t.disk = j.text
      if (!dirtyTab(t)) {
        t.saved = j.text; t.text = j.text
        if (t === tab()) { const { a, b } = ed; ed.value = j.text; ed.a = Math.min(a, j.text.length); ed.b = Math.min(b, j.text.length); ed.paint() }
      } else if (t === tab()) fail(`${t.path} changed on disk; saving here writes over it`)
    } catch {}
  }
  renderTabs()
  listFiles()
})
addEventListener('beforeunload', (e) => { stash(); if (tabs.some(dirtyTab)) { e.preventDefault(); e.returnValue = '' } })

// ---------- the files ----------

let files = [], libFiles = [], pick = 0
const shut = new Set(store.get('shut', []))
async function listFiles() {
  try { const j = await api('ls'); files = j.files.map((f) => f.path).sort(); $('#root').textContent = j.root } catch (e) { files = []; $('#root').textContent = e.message }
  showList()
}
function picked() {
  const q = $('#filter').value.trim().toLowerCase()
  return [...files, ...libFiles].filter((f) => f.toLowerCase().includes(q))
}
function showList() {
  const l = $('#list')
  l.replaceChildren()
  const q = $('#filter').value.trim()
  const row = (text, cls, title, on, pad = 0) => {
    const d = document.createElement('div')
    d.textContent = text; d.className = cls; d.title = title
    d.style.paddingLeft = 12 + pad * 12 + 'px'
    d.onclick = on
    l.append(d)
    return d
  }
  const curPath = tab() && tab().path
  if (q) {
    const fs = picked()
    pick = Math.min(Math.max(pick, 0), fs.length - 1)
    fs.forEach((f, i) => row(f, (f === curPath ? 'cur ' : '') + (i === pick ? 'sel ' : '') + (isLib(f) ? 'std' : ''), f, () => openFile(f)))
    return
  }
  // a tree: folders before files, folders can be shut
  const tree = (list, base, pad) => {
    const dirs = new Map(), here = []
    for (const f of list) {
      const rest = f.slice(base.length)
      const k = rest.indexOf('/')
      if (k < 0) here.push(f)
      else { const d = rest.slice(0, k); if (!dirs.has(d)) dirs.set(d, []); dirs.get(d).push(f) }
    }
    for (const [d, fs] of dirs) {
      const key = base + d + '/'
      const r = row(d, 'dir' + (shut.has(key) ? ' shut' : '') + (isLib(key) ? ' std' : ''), key, () => { shut.has(key) ? shut.delete(key) : shut.add(key); store.set('shut', [...shut]); showList() }, pad)
      void r
      if (!shut.has(key)) tree(fs, key, pad + 1)
    }
    for (const f of here) row(f.slice(base.length), (f === curPath ? 'cur ' : '') + (isLib(f) ? 'std' : ''), f, () => openFile(f), pad)
  }
  tree([...files, ...libFiles], '', 0)
}
function openPicker() { $('#files').hidden = false; store.set('files', true); $('#filter').focus(); $('#filter').select() }
$('#filter').addEventListener('input', () => { pick = 0; showList() })
$('#filter').addEventListener('keydown', (e) => {
  const fs = picked()
  if (e.key === 'ArrowDown') { pick = Math.min(pick + 1, fs.length - 1); showList(); e.preventDefault() }
  else if (e.key === 'ArrowUp') { pick = Math.max(pick - 1, 0); showList(); e.preventDefault() }
  else if (e.key === 'Enter') {
    const q = $('#filter').value.trim()
    if (/\.s$/.test(q) && !files.includes(q) && !isLib(q)) newFile(q)
    else if (fs[pick]) openFile(fs[pick])
    $('#filter').value = ''; showList()
    e.preventDefault()
  } else if (e.key === 'Escape') { $('#filter').value = ''; showList(); ed.focus() }
})

// ---------- messages ----------

let msgUntil = 0, msgAt = -1   // a message stays while the caret stays
function fail(msg, at = null) {
  msgUntil = Date.now() + 6000; msgAt = ed.b
  foot.textContent = msg
  foot.className = 'foot err'
  foot.onclick = at !== null ? () => ed.goTo(at) : null
  clearTimeout(noteT)
  noteT = setTimeout(caretInfo, 6050)
}
let noteT = 0
function note(msg) {
  msgUntil = Date.now() + 2500; msgAt = ed.b
  foot.textContent = msg
  foot.className = 'foot'
  foot.onclick = null
  clearTimeout(noteT)
  noteT = setTimeout(caretInfo, 2550)
}

// ---------- running ----------

let runTimer = 0, running = false
function clearRun() {
  ed.joins = []; ed.forks = []; ed.err = null
  ed.setNotes(new Map())
  ed.render()
}
/** Arguments as typed: words, or 'quoted words' kept together. */
function argv() {
  const s = $('#args').value
  const out = []
  const re = /'([^']*)'|(\S+)/g
  let m
  while ((m = re.exec(s))) out.push(m[1] !== undefined ? m[1] : m[2])
  return out
}
function report(j, label) {
  const src = ed.value
  out.replaceChildren()
  const head = document.createElement('span')
  head.className = 'head'
  head.textContent = label
  out.append(head)
  if (j.out) out.append(document.createTextNode(j.out.replace(/^\n/, '')))
  if (j.stderr) { const s = document.createElement('span'); s.className = 'note'; s.textContent = j.stderr; out.append(s) }
  const notes = new Map()
  // what each line printed, beside it
  const starts = [0]
  for (let i = 0; i < src.length; i++) if (src[i] === '\n') starts.push(i + 1)
  const lineAt = (x) => { let lo = 0, hi = starts.length - 1; while (lo < hi) { const m = (lo + hi + 1) >> 1; if (starts[m] <= x) lo = m; else hi = m - 1 } return lo }
  for (const t of j.tags || []) {
    const s = t.text.replace(/\s+/g, ' ').trim()
    if (!s) continue
    const li = lineAt(jsIndex(src, t.at))
    const was = notes.get(li)
    const text = (was ? was.text + ' · ' : '') + s
    notes.set(li, { text, cls: /(^|· )fail\b/.test(text) ? 'fail' : '' })
  }
  ed.err = null
  if (j.error && j.error.msg) {
    const at = j.error.at != null ? jsIndex(src, j.error.at) : null
    const s = document.createElement('span')
    s.className = 'err'
    s.textContent = '× ' + j.error.msg + (at !== null ? '  · line ' + lineOf(src, at) : '')
    if (at !== null) { s.onclick = () => ed.goTo(at); notes.set(lineAt(at), { text: '× ' + j.error.msg, cls: 'err' }) }
    out.append(s)
    ed.err = at
  }
  if (j.exit != null && j.exit !== 0) { const s = document.createElement('span'); s.className = 'note'; s.textContent = 'exit ' + j.exit; out.append(s) }
  const forks = (j.forks || []).map((f) => ({ at: jsIndex(src, f.at), how: f.how, n: f.n, lines: f.lines || [] }))
  const last = new Map()
  for (const f of forks) last.set(f.at, f)
  ed.joins = [...last.values()].map((f) => ({ at: f.at, n: f.n, lines: f.lines }))
  ed.forks = [...last.values()]
  ed.notes = notes
  redraw()
  if (j.error && j.error.msg) fail(j.error.msg, ed.err)
  const fl = $('#forks')
  fl.replaceChildren()
  for (const f of last.values()) {
    const d = document.createElement('div')
    d.innerHTML = `<span class="ln">line ${lineOf(src, f.at)}</span><span></span>`
    d.lastChild.textContent = f.how
    d.onclick = () => ed.goTo(f.at)
    fl.append(d)
  }
  rtab('out')
}
async function run(quiet = false) {
  clearTimeout(runTimer)
  const t = tab()
  if (running || !t) return
  running = true
  $('#stats').textContent = '…'
  try {
    const j = await api('srun', { text: ed.value, path: t.lib ? '' : t.path, trace: true, ms: quiet ? 3000 : 20000, args: argv(), stdin: $('#stdin').value })
    report(j, `▸ ${t.path} · machine`)
    $('#stats').textContent = `${j.ops} ops · ${j.ms.toFixed(1)} ms`
  } catch (e) { fail(e.message); $('#stats').textContent = '' }
  running = false
}
async function build() {
  clearTimeout(runTimer)
  const t = tab()
  if (running || !t) return
  running = true
  $('#stats').textContent = 'compiling to Rust and building…'
  try {
    const j = await api('build', { text: ed.value, path: t.lib ? 'build.s' : t.path, ms: 60000, args: argv(), stdin: $('#stdin').value, fold: $('#fold').checked })
    report(j, `▸ ${t.path} · compiled`)
    if (j.how) { const s = document.createElement('span'); s.className = 'note'; s.textContent = j.how; out.insertBefore(s, out.firstChild.nextSibling) }
    $('#stats').textContent = `ran ${j.ms.toFixed(1)} ms` + (j.built ? ` · built in ${(j.build_ms / 1000).toFixed(1)} s` : ' · built before')
  } catch (e) { fail(e.message); $('#stats').textContent = ''; out.replaceChildren(Object.assign(document.createElement('span'), { className: 'err', textContent: e.message })); rtab('out') }
  running = false
}
async function rust() {
  const t = tab()
  if (!t) return
  rtab('rust')
  $('#rs').textContent = 'compiling…'
  try { const j = await api('rustsrc', { text: ed.value, path: t.path, fold: $('#fold').checked }); $('#rs').textContent = (j.note ? '// ' + j.note + '\n' : '') + j.rs }
  catch (e) { $('#rs').textContent = e.message }
}
$('#rsget').onclick = rust
$('#rscopy').onclick = () => { navigator.clipboard && navigator.clipboard.writeText($('#rs').textContent); note('the Rust is copied') }

// the console: s with the file's definitions
const hist = store.get('hist', [])
let histAt = hist.length
$('#cmd').addEventListener('keydown', async (e) => {
  if (e.key === 'ArrowUp') { histAt = Math.max(0, histAt - 1); $('#cmd').value = hist[histAt] || ''; e.preventDefault(); return }
  if (e.key === 'ArrowDown') { histAt = Math.min(hist.length, histAt + 1); $('#cmd').value = hist[histAt] || ''; e.preventDefault(); return }
  if (e.key === 'Escape') { ed.focus(); return }
  if (e.key !== 'Enter') return
  const line = $('#cmd').value.trim()
  if (!line) return
  if (hist[hist.length - 1] !== line) hist.push(line)
  while (hist.length > 200) hist.shift()
  store.set('hist', hist)
  histAt = hist.length
  const t = tab()
  try {
    const j = await api('srun', { text: line, prelude: prelude(ed.value), path: t && !t.lib ? t.path : '', trace: false, ms: 10000, args: argv(), stdin: $('#stdin').value })
    const h = document.createElement('span'); h.className = 'head'; h.textContent = '› ' + line
    out.append(h)
    if (j.out) out.append(document.createTextNode(j.out.replace(/^\n/, '') + '\n'))
    if (j.error && j.error.msg) { const s = document.createElement('span'); s.className = 'err'; s.textContent = '× ' + j.error.msg; out.append(s) }
    out.scrollTop = 1e9
  } catch (er) { fail(er.message) }
  $('#cmd').select()
})

// ---------- moving: definitions, uses, back ----------

const back = []
function wordAtX(x) {
  const [a, b] = ed.wordAt(x)
  if (a === b) { const [c, d] = ed.wordAt(Math.max(0, x - 1)); return { a: c, b: d, w: ed.value.slice(c, d) } }
  return { a, b, w: ed.value.slice(a, b) }
}
function resolve(from, rel) {
  if (isLib(rel)) return rel
  const dir = from && from.includes('/') ? from.slice(0, from.lastIndexOf('/') + 1) : ''
  const norm = []
  for (const x of (dir + rel).split('/')) { if (x === '..') norm.pop(); else if (x && x !== '.') norm.push(x) }
  return norm.join('/')
}
/** The files a text loads with name.s. */
function loads(text, from) {
  return [...new Set(tokenize(text).filter((t) => /\.s\.$/.test(t.w)).map((t) => resolve(from, bare(t.w).replace(/\.$/, '') )))]
}
async function textOf(p) {
  const t = tabs.find((x) => x.path === p)
  if (t) return t === tab() ? ed.value : t.text
  try { return (isLib(p) ? await api('lib', { path: p }) : await api('read', { path: p })).text } catch { return null }
}
async function goDef(x = ed.b) {
  const t = tab()
  if (!t) return
  const { a, w } = wordAtX(x)
  if (!w) return
  const go = async (p, d) => { back.push({ path: t.path, a: ed.a, b: ed.b }); if (p === t.path) { ed.select(d.from, d.to); ed.focus() } else await openFile(p, [d.from, d.to]) }
  const m = w.match(/^(.+\.s)\.$/)
  if (m) { back.push({ path: t.path, a: ed.a, b: ed.b }); if (!(await openFile(resolve(t.path, m[1])))) back.pop(); return }
  const name = bare(w)
  const here = defsOf(ed.value).get(name)
  if (here) { const d = here.find((d) => d.from !== a) || here[0]; return go(t.path, d) }
  // a file this one loads, and the files those load
  const seen = new Set([t.path])
  const todo = loads(ed.value, t.path)
  while (todo.length) {
    const p = todo.shift()
    if (seen.has(p)) continue
    seen.add(p)
    const s = await textOf(p)
    if (s == null) continue
    const d = defsOf(s).get(name)
    if (d) return go(p, d[0])
    todo.push(...loads(s, p))
  }
  const doc = docs.get(name)
  if (doc && doc.file) {
    const s = await textOf(doc.file)
    const d = s && defsOf(s).get(name)
    if (d) return go(doc.file, d[0])
  }
  if (doc && doc.kind === 'cmd') note(`${name} is a command: ${doc.sig}  ${doc.doc}`)
  else note(`${name}: no definition in this file or the files it loads`)
}
async function goBack() {
  const b = back.pop()
  if (!b) return
  if (!tab() || b.path !== tab().path) await openFile(b.path)
  ed.select(b.a, b.b); ed.focus()
}
function uses(x = ed.b) {
  const { w } = wordAtX(x)
  const name = bare(w)
  if (!name) return
  const src = ed.value
  const hits = tokenize(src).filter((t) => t.kind !== 'open' && bare(t.kind === 'close' ? t.w.slice(1) : t.w) === name && !/^-?\d/.test(name)).map((t) => ({ a: t.kind === 'close' ? t.from + 1 : t.from, b: t.to }))
  ed.setMarks(hits.map((h) => ({ ...h, cls: 'use' })))
  found(`${hits.length} use${hits.length === 1 ? '' : 's'} of ${name}`, hits.map((h) => ({ path: tab().path, a: h.a, b: h.b, text: src })))
}
function outline() {
  const src = ed.value
  const ds = [...defsOf(src)].flatMap(([n, l]) => l.map((d) => ({ n, ...d }))).sort((x, y) => x.from - y.from)
  found(`${ds.length} definition${ds.length === 1 ? '' : 's'} in ${tab().path}`, ds.map((d) => ({ path: tab().path, a: d.from, b: d.to, text: src, label: d.n })))
}
/** Results in the found tab: { path, a, b, text, label? }. */
function found(title, items) {
  rtab('found')
  $('#fhead').replaceChildren(fileSearch, document.createTextNode(' ' + title))
  const l = $('#flist')
  l.replaceChildren()
  for (const it of items.slice(0, 2000)) {
    const d = document.createElement('div')
    const ls = it.text.lastIndexOf('\n', it.a - 1) + 1
    const le = it.text.indexOf('\n', it.a)
    const line = it.text.slice(ls, le < 0 ? it.text.length : le)
    const at = document.createElement('span')
    at.className = 'at'
    at.textContent = (it.path === (tab() && tab().path) ? '' : it.path + ':') + lineOf(it.text, it.a)
    const tx = document.createElement('span')
    const p = it.a - ls, q = it.b - ls
    tx.innerHTML = it.label ? `<b>${esc(it.label)}</b>  ${esc(line.trim())}` : esc(line.slice(0, p)).trimStart() + `<span class="hit">${esc(line.slice(p, q))}</span>` + esc(line.slice(q))
    d.append(at, tx)
    d.onclick = () => openFile(it.path, [it.a, it.b])
    l.append(d)
  }
}
const fileSearch = document.createElement('input')
fileSearch.placeholder = 'find in every file'
fileSearch.spellcheck = false
fileSearch.addEventListener('keydown', async (e) => {
  if (e.key === 'Escape') { ed.focus(); return }
  if (e.key !== 'Enter') return
  const q = fileSearch.value
  if (!q) return
  const items = []
  await listFiles()
  for (const p of files) {
    const s = await textOf(p)
    if (s == null) continue
    let i = s.indexOf(q)
    while (i >= 0) { items.push({ path: p, a: i, b: i + q.length, text: s }); i = s.indexOf(q, i + q.length) }
  }
  found(`${items.length} in ${new Set(items.map((x) => x.path)).size} files`, items)
  fileSearch.focus()
})

// ---------- find and replace ----------

const find = { open: false, hits: [], at: -1, cs: store.get('qcase', false), re: store.get('qre', false) }
$('#qcase').classList.toggle('on', find.cs)
$('#qre').classList.toggle('on', find.re)
function openFind(replace) {
  find.open = true
  $('#find').hidden = false
  const s = ed.selected()
  if (s && !s.includes('\n')) $('#q').value = s
  ;(replace ? $('#rep') : $('#q')).focus()
  $('#q').select()
  findHits()
}
function closeFind() { find.open = false; $('#find').hidden = true; ed.setMarks([]); ed.focus() }
function findHits() {
  const q = $('#q').value, src = ed.value
  find.hits = []
  if (q) {
    try {
      const re = new RegExp(find.re ? q : q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), find.cs ? 'g' : 'gi')
      let m
      while ((m = re.exec(src))) { if (!m[0].length) { re.lastIndex++; continue } find.hits.push({ a: m.index, b: m.index + m[0].length }) }
      $('#q').style.color = ''
    } catch { $('#q').style.color = 'var(--err)' }
  }
  find.at = find.hits.findIndex((h) => h.a >= Math.min(ed.a, ed.b))
  paintHits()
}
function paintHits() {
  ed.setMarks(find.hits.map((h, i) => ({ ...h, cls: i === find.at ? 'cur' : '' })))
  $('#qn').textContent = find.hits.length ? `${find.at + 1} of ${find.hits.length}` : $('#q').value ? 'none' : ''
}
function findStep(d) {
  if (!find.hits.length) return
  const x = Math.min(ed.a, ed.b)
  if (find.at < 0 || !(find.hits[find.at] && find.hits[find.at].a === x)) {
    find.at = d > 0 ? find.hits.findIndex((h) => h.a > x - (ed.a === ed.b ? 1 : 0)) : find.hits.length - 1 - [...find.hits].reverse().findIndex((h) => h.a < x)
    if (find.at < 0 || find.at >= find.hits.length) find.at = d > 0 ? 0 : find.hits.length - 1
  } else find.at = (find.at + d + find.hits.length) % find.hits.length
  const h = find.hits[find.at]
  ed.select(h.a, h.b)
  paintHits()
}
function replaceWith(m) {
  const r = $('#rep').value
  if (!find.re) return r
  return m.replace(new RegExp($('#q').value, find.cs ? '' : 'i'), r)
}
$('#q').addEventListener('input', findHits)
$('#q').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') { findStep(e.shiftKey ? -1 : 1); e.preventDefault() }
  else if (e.key === 'Escape') closeFind()
})
$('#rep').addEventListener('keydown', (e) => {
  if (e.key === 'Enter') { $('#rone').click(); e.preventDefault() }
  else if (e.key === 'Escape') closeFind()
})
$('#qcase').onclick = () => { find.cs = !find.cs; store.set('qcase', find.cs); $('#qcase').classList.toggle('on', find.cs); findHits() }
$('#qre').onclick = () => { find.re = !find.re; store.set('qre', find.re); $('#qre').classList.toggle('on', find.re); findHits() }
$('#qnext').onclick = () => findStep(1)
$('#qprev').onclick = () => findStep(-1)
$('#qclose').onclick = closeFind
$('#rone').onclick = () => {
  const h = find.hits[find.at]
  if (!h || ed.readOnly) return findStep(1)
  if (Math.min(ed.a, ed.b) !== h.a || Math.max(ed.a, ed.b) !== h.b) return findStep(1)
  const r = replaceWith(ed.value.slice(h.a, h.b))
  ed.edit(h.a, h.b, r, h.a + r.length)
  findHits()
  findStep(1)
}
$('#rall').onclick = () => {
  if (ed.readOnly || !find.hits.length) return
  let s = ed.value, n = 0
  for (const h of [...find.hits].reverse()) { s = s.slice(0, h.a) + replaceWith(s.slice(h.a, h.b)) + s.slice(h.b); n++ }
  ed.edit(0, ed.value.length, s, Math.min(ed.b, s.length))
  findHits()
  note(`replaced ${n}`)
}

// ---------- completion ----------

const comp = { items: [], i: 0, s: 0, e: 0, open: false }
function wordBefore() {
  const t = ed.value, e = ed.b
  let s = e
  while (s > 0 && !/[\s{}⟪¦⟫⨝']/.test(t[s - 1])) s--
  return { s, e, w: t.slice(s, e) }
}
function complete(force = false) {
  if (ed.readOnly || ed.a !== ed.b) return closeComp()
  const { s, e, w } = wordBefore()
  if (!force && w.length < 2) return closeComp()
  const lead = w.startsWith('\\') ? '\\' : ''
  const stem = w.slice(lead.length)
  const seen = new Set(), items = []
  const add = (word, note, rank) => { if (word.startsWith(stem) && word !== stem && !seen.has(word)) { seen.add(word); items.push({ word: lead + word, note, rank }) } }
  for (const [n, ds] of defsOf(ed.value)) add(n, 'defined on line ' + lineOf(ed.value, ds[0].from), 0)
  for (const [word, d] of docs) add(word, (d.sig || '') + (d.file ? '  ' + d.file : '') || d.doc, d.kind === 'cmd' ? 1 : 2)
  for (const t of tokenize(ed.value)) if (t.kind === 'word' && t.w.length > 2 && !/^-?\d/.test(t.w)) { add(t.w, '', 3); add(bare(t.w), '', 3) }
  items.sort((x, y) => x.rank - y.rank || x.word.length - y.word.length || (x.word < y.word ? -1 : 1))
  comp.items = items.slice(0, 50)
  comp.i = 0; comp.s = s; comp.e = e
  if (!comp.items.length) return closeComp()
  comp.open = true
  renderComp()
}
function renderComp() {
  const el = $('#comp')
  el.hidden = false
  el.replaceChildren()
  comp.items.forEach((it, k) => {
    const d = document.createElement('div')
    d.className = k === comp.i ? 'on' : ''
    d.append(document.createTextNode(it.word))
    const sp = document.createElement('span'); sp.textContent = it.note.slice(0, 80)
    d.append(sp)
    d.onmousedown = (e) => { e.preventDefault(); comp.i = k; takeComp() }
    el.append(d)
  })
  const c = ed.caretRect(ed.b), r = ed.inner.getBoundingClientRect()
  const x = c.left + r.left, y = c.top + r.top + c.height + 2
  el.style.top = Math.min(y, innerHeight - el.offsetHeight - 6) + 'px'
  el.style.left = Math.max(6, Math.min(x - el.offsetWidth / 2, innerWidth - el.offsetWidth - 6)) + 'px'
  const on = el.children[comp.i]
  if (on) on.scrollIntoView({ block: 'nearest' })
}
function takeComp() {
  const it = comp.items[comp.i]
  closeComp()
  if (!it) return
  const { s, e } = wordBefore()
  ed.edit(s, e, it.word, s + it.word.length)
}
function closeComp() { comp.open = false; comp.items = []; $('#comp').hidden = true }
/** Keys the editor would take for itself (Alt+←, Alt+PageUp/Down) that move between places. */
function edKey(e) {
  if (!e.altKey) return false
  if (e.key === 'ArrowLeft') { goBack(); return true }
  if (e.key === 'PageUp') { if (tabs.length) show((cur - 1 + tabs.length) % tabs.length); return true }
  if (e.key === 'PageDown') { if (tabs.length) show((cur + 1) % tabs.length); return true }
  return false
}
function compKey(e) {
  if ((e.ctrlKey || e.metaKey) && e.code === 'Space') { complete(true); return true }
  if (!comp.open) return false
  if (e.key === 'ArrowDown') { comp.i = (comp.i + 1) % comp.items.length; renderComp(); return true }
  if (e.key === 'ArrowUp') { comp.i = (comp.i - 1 + comp.items.length) % comp.items.length; renderComp(); return true }
  if (e.key === 'Tab' || e.key === 'Enter') { takeComp(); return true }
  if (e.key === 'Escape') { closeComp(); return true }
  return false
}

// ---------- what a word is, under the pointer ----------

function describe(w, at) {
  const parts = []
  const name = bare(w)
  const d = docs.get(w) || docs.get(name)
  if (d) parts.push(`<b>${esc(d.sig || w)}</b>${d.file ? '  <i>' + esc(d.file) + '</i>' : d.kind === 'cmd' ? '  <i>command</i>' : ''}\n${esc(d.doc)}`)
  const last = w[w.length - 1]
  if (w.length > 1 && SUFFIX.includes(last) && caps.has(last) && !docs.has(w)) {
    const c = caps.get(w.endsWith('..') ? '..' : last)
    if (c) parts.push(`<b>${esc(name)}</b> · ${esc(c.name)}: ${esc(c.doc)}`)
  }
  if (w.startsWith('\\') && docs.has(w.slice(1))) parts.push('the command itself, past any replacement')
  if (/\.s\.$/.test(w)) parts.push('loads ' + esc(w.slice(0, -1)) + ' · F12 opens it')
  const defs = defsOf(ed.value).get(name)
  if (defs) {
    const own = defs.find((x) => x.from <= at && at <= x.to)
    parts.push(own ? 'defined here' : `defined on line ${lineOf(ed.value, defs[0].from)} · F12 goes there`)
  }
  return parts.join('\n')
}
function hover(x, cx, cy) {
  const tip = $('#tip')
  if (x === null || x === undefined || comp.open) { tip.hidden = true; return }
  const { w, a } = wordAtX(x)
  const h = w && describe(w, a)
  if (!h) { tip.hidden = true; return }
  tip.innerHTML = h
  tip.hidden = false
  tip.style.left = Math.max(6, Math.min(cx - tip.offsetWidth / 2, innerWidth - tip.offsetWidth - 6)) + 'px'
  tip.style.top = (cy + 18 + tip.offsetHeight > innerHeight ? cy - tip.offsetHeight - 14 : cy + 18) + 'px'
}

// ---------- the palette ----------

function renderPalette() {
  const el = $('#palette')
  el.replaceChildren()
  const btn = (w, cls, title, on) => {
    const b = document.createElement('button')
    b.textContent = w; b.className = cls; b.title = title
    b.onmousedown = (e) => e.preventDefault()
    b.onclick = on
    return b
  }
  for (const g of groups) {
    const r = document.createElement('div')
    r.className = 'grp'
    const h = document.createElement('b'); h.textContent = g.name
    r.append(h)
    for (const it of g.items) r.append(btn(it.w, it.kind, (it.sig ? it.sig + '\n' : '') + (it.doc || ''), () => insertWord(it.w)))
    el.append(r)
  }
  const r = document.createElement('div')
  r.className = 'grp'
  const h = document.createElement('b'); h.textContent = 'sigils'
  r.append(h)
  for (const [s, c] of caps) r.append(btn(s, 'cap', c.name + ': ' + c.doc, () => insertSigil(s)))
  el.append(r)
}
function insertWord(w) {
  if (ed.readOnly) return
  const t = ed.value, [x, y] = ed.sel()
  const pre = x > 0 && !/\s/.test(t[x - 1]) ? ' ' : ''
  const post = y < t.length && !/\s/.test(t[y]) ? ' ' : ''
  ed.edit(x, y, pre + w + post, x + pre.length + w.length)
  ed.focus()
}
function insertSigil(s) {
  if (ed.readOnly) return
  const x = ed.b
  ed.edit(x, x, s, x + s.length)
  ed.focus()
}
function togglePalette() { $('#palette').hidden = !$('#palette').hidden; store.set('palette', !$('#palette').hidden); ed.paint(); ed.lines() }

// ---------- the caret and edits ----------

let sizeTimer = 0, sizes = ''
function caretInfo() {
  if (Date.now() < msgUntil && ed.b === msgAt) return
  msgUntil = 0
  const t = tab()
  if (!t) { foot.textContent = 'Ctrl+P opens a file, or type name.s and Enter to make one'; foot.className = 'foot'; return }
  const src = ed.value
  const ls = src.lastIndexOf('\n', ed.b - 1) + 1
  const st = ed.stats()
  const lanes = st && st.forks ? `${st.forks} fork${st.forks > 1 ? 's' : ''} · ${st.lanes} lanes · mints ${st.minted} · ` : ''
  const { w } = wordAtX(ed.b)
  const d = w && (docs.get(w) || docs.get(bare(w)))
  const what = d ? `${d.sig || w} · ` : ''
  foot.className = 'foot'
  foot.onclick = null
  foot.textContent = `${what}${lanes}line ${lineOf(src, ed.b)}, col ${ed.b - ls + 1} · ${sizes}${t.lib ? ' · std, read-only' : ''}`
  if (ed.mint) ed.showMint()
  if (!t.lib) store.set('caret:' + t.path, ed.b)
}
function measure() {
  const t = ed.value
  sizes = `${kb(utf8(t))} utf-8 · ${kb(encode(t).length)} s5`
  caretInfo()
}
function changed() {
  renderTabs()
  if (ed.notes.size || ed.forks.length) { ed.joins = []; ed.forks = []; ed.notes = new Map(); ed.render() }
  if (find.open) findHits()
  clearTimeout(sizeTimer)
  sizeTimer = setTimeout(measure, 200)
  const { w } = wordBefore()
  if (w.length >= 2 && document.activeElement === ed.input) complete(); else closeComp()
  if ($('#live').checked) { clearTimeout(runTimer); runTimer = setTimeout(() => run(true), 500) }
}

// ---------- the right side ----------

function rtab(name) {
  for (const b of document.querySelectorAll('#rtabs button')) b.classList.toggle('on', b.dataset.tab === name)
  for (const el of document.querySelectorAll('#right .pane')) el.hidden = el.id !== 't-' + name
}
for (const b of document.querySelectorAll('#rtabs button')) b.onclick = () => { rtab(b.dataset.tab); if (b.dataset.tab === 'found' && !$('#flist').children.length) found('', []) }

// ---------- keys ----------

addEventListener('keydown', (e) => {
  const ctrl = e.ctrlKey || e.metaKey, k = e.key.toLowerCase()
  const take = () => { e.preventDefault(); e.stopPropagation() }
  if (ctrl && e.key === 'Enter') { take(); e.shiftKey ? build() : run() }
  else if (ctrl && k === 's') { take(); save() }
  else if (ctrl && k === 'p') { take(); openPicker() }
  else if (ctrl && e.shiftKey && k === 'f') { take(); found('', []); fileSearch.focus() }
  else if (ctrl && k === 'f') { take(); openFind(false) }
  else if (ctrl && k === 'h') { take(); openFind(true) }
  else if (e.key === 'F3') { take(); if (!find.open) openFind(false); findStep(e.shiftKey ? -1 : 1) }
  else if (e.key === 'F12') { take(); e.shiftKey ? uses() : goDef() }
  else if (e.altKey && e.key === 'ArrowLeft') { take(); goBack() }
  else if (ctrl && e.shiftKey && k === 'o') { take(); outline() }
  else if (ctrl && k === 'g') { take(); const n = parseInt(prompt('line') || '', 10); if (n > 0) ed.goTo(ed.lineAt(n - 1)) }
  else if (ctrl && k === 'r') { take(); rust() }
  else if (ctrl && e.key === '`') { take(); rtab('out'); $('#cmd').focus() }
  else if (ctrl && (e.key === '=' || e.key === '+')) { take(); zoom(1) }
  else if (ctrl && e.key === '-') { take(); zoom(-1) }
  else if (ctrl && e.key === '0') { take(); zoom(0) }
  else if (e.altKey && k === 'w') { take(); closeTab(cur) }
  else if (e.altKey && e.key === 'PageUp') { take(); if (tabs.length) show((cur - 1 + tabs.length) % tabs.length) }
  else if (e.altKey && e.key === 'PageDown') { take(); if (tabs.length) show((cur + 1) % tabs.length) }
  else if (e.key === 'F1') { take(); $('#keys').hidden = !$('#keys').hidden }
  else if (e.key === 'F2') { take(); togglePalette() }
  else if (e.key === 'Escape') {
    if (!$('#keys').hidden) $('#keys').hidden = true
    else if (find.open && document.activeElement === ed.input) closeFind()
    else if (ed.marks.length) ed.setMarks([])
    $('#tip').hidden = true
  }
})
let fs = store.get('fs', 14)
function zoom(d) { fs = d === 0 ? 14 : Math.max(9, Math.min(28, fs + d)); store.set('fs', fs); ed.setZoom(fs) }

$('#save') && ($('#save').onclick = save)
$('#run').onclick = () => run()
$('#build').onclick = build
$('#ref').onclick = () => { $('#keys').hidden = !$('#keys').hidden }
$('#pal').onclick = togglePalette
$('#mint').onclick = () => { ed.toggleMint(); ed.focus() }
$('#toggleFiles').onclick = () => { $('#files').hidden = !$('#files').hidden; store.set('files', !$('#files').hidden); ed.paint(); ed.lines() }
$('#files').hidden = !store.get('files', true)
$('#palette').hidden = !store.get('palette', false)
$('#live').checked = store.get('live', false)
$('#fold').checked = store.get('fold', true)
$('#fold').onchange = () => store.set('fold', $('#fold').checked)
$('#live').onchange = () => store.set('live', $('#live').checked)
$('#braces').checked = store.get('braces', false)
ed.showBraces($('#braces').checked)
$('#braces').onchange = () => { store.set('braces', $('#braces').checked); ed.showBraces($('#braces').checked); ed.focus() }
$('#args').addEventListener('change', stash)
$('#stdin').addEventListener('change', stash)

$('#split').addEventListener('pointerdown', (e) => {
  const move = (ev) => { $('#right').style.width = Math.max(220, Math.min(innerWidth - 420, innerWidth - ev.clientX)) + 'px' }
  const up = () => { removeEventListener('pointermove', move); removeEventListener('pointerup', up); store.set('right', $('#right').style.width); ed.paint(); ed.lines() }
  addEventListener('pointermove', move)
  addEventListener('pointerup', up)
  e.preventDefault()
})
if (store.get('right', null)) $('#right').style.width = store.get('right')

;(async () => {
  ready = true
  ed.setZoom(fs)
  ed.setReadOnly(true)
  await loadDocs()
  renderPalette()
  try { libFiles = (await api('lib')).files || [] } catch {}
  await listFiles()
  for (const p of store.get('tabs', [])) if (files.includes(p)) await openFile(p)
  const last = store.get('cur', null)
  const i = tabs.findIndex((t) => t.path === last)
  if (i >= 0) show(i)
  if (!tabs.length) { openPicker(); caretInfo() }
})()
