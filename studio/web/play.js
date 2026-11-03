// The playground: s on the left in the editor, the page it describes on the right.
import { Editor } from './ed.js'
import { compilePage } from './compile.js'
import { LIB } from './std.js'
import { encode } from './s5.js'

const $ = (s) => document.querySelector(s)
const view = $('#view'), out = $('#out'), status = $('#status'), where = $('#where')
const store = {
  get(k, d) { try { const v = localStorage.getItem('play.' + k); return v == null ? d : JSON.parse(v) } catch { return d } },
  set(k, v) { try { localStorage.setItem('play.' + k, JSON.stringify(v)) } catch {} },
}
let ready = false, pending = null, runTimer = 0
const utf8 = (s) => new TextEncoder().encode(s).length
const kb = (n) => (n < 1024 ? n + ' B' : (n / 1024).toFixed(1) + ' KB')
const jsIndex = (s, cp) => { let i = 0, n = 0; for (const ch of s) { if (n === cp) return i; i += ch.length; n++ } return i }

let ed = null
ed = new Editor($('#ed'), {
  onChange: () => changed(),
  onCaret: () => caretInfo(),
})
let sizeTimer = 0, sizes = ''
function caretInfo() {
  if (!ed) return
  const st = ed.stats()
  const lanes = st && st.forks ? `${st.forks} fork${st.forks > 1 ? 's' : ''} · ${st.lanes} lanes · mints ${st.minted} · ` : ''
  where.textContent = lanes + sizes
  if (ed.mint) ed.showMint()
}
function measure() {
  const t = ed.value
  sizes = `${kb(utf8(t))} utf-8 · ${kb(encode(t).length)} s5`
  caretInfo()
}

// ---------- running ----------

function run() {
  clearTimeout(runTimer)
  out.textContent = ''
  ed.setError(null)
  status.textContent = ''
  status.className = ''
  const msg = { src: ed.value }
  if (ready) view.contentWindow.postMessage(msg, '*')
  else pending = msg
}
addEventListener('message', (e) => {
  if (e.source !== view.contentWindow) return
  const d = e.data || {}
  if (d.ready) { if (ready) return; ready = true; if (pending) { view.contentWindow.postMessage(pending, '*'); pending = null } else run(); return }
  if (d.out != null) out.append(document.createTextNode(d.out.replace(/^\n/, out.textContent ? '\n' : '')))
  if (d.err) {
    const src = ed.value
    const at = d.at == null ? null : jsIndex(src, d.at)
    const line = document.createElement('span')
    line.className = 'err'
    line.textContent = '× ' + d.err + (at != null ? '  · line ' + (src.slice(0, at).split('\n').length) : '')
    if (at != null) line.onclick = () => ed.goTo(at)
    out.append(line)
    status.textContent = d.err
    status.className = 'err'
    status.onclick = at != null ? () => ed.goTo(at) : null
    if (at != null) ed.setError(at)
  }
  if (d.done) {
    $('#ops').textContent = `· ${d.ops} ops · ${d.ms.toFixed(1)} ms`
    const src = ed.value
    ed.setJoins((d.joins || []).map(([at, lines]) => ({ at: jsIndex(src, at), lines })))
  }
})

// ---------- editing ----------

function setSource(text) {
  ed.value = text
  ed.goTo(0)
  changed()
  run()
}
function changed() {
  store.set('src', ed.value)
  showName()
  clearTimeout(sizeTimer)
  sizeTimer = setTimeout(measure, 200)
  if ($('#live').checked) { clearTimeout(runTimer); runTimer = setTimeout(run, 450) }
}
addEventListener('keydown', (e) => {
  const ctrl = e.ctrlKey || e.metaKey
  if (ctrl && e.key === 'Enter') { e.preventDefault(); run() }
  else if (ctrl && e.key.toLowerCase() === 's') { e.preventDefault(); save() }
  else if (ctrl && e.key.toLowerCase() === 'o') { e.preventDefault(); open() }
  else if (e.key === 'F1') { e.preventDefault(); $('#words').hidden = !$('#words').hidden }
  else if (e.key === 'Escape') $('#words').hidden = true
})
$('#braces').checked = store.get('braces', false)
ed.showBraces($('#braces').checked)
$('#braces').onchange = () => { store.set('braces', $('#braces').checked); ed.showBraces($('#braces').checked); ed.focus() }
$('#mint').onclick = () => { ed.toggleMint(); ed.focus() }

// ---------- files ----------

const name = () => store.get('name', 'page')
function download(file, data, type) {
  const a = document.createElement('a')
  a.href = URL.createObjectURL(new Blob([data], { type }))
  a.download = file
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 1000)
}
// the file the program came from: saved in place where the browser can, downloaded where not
let handle = null, saved = store.get('saved', '')
const canPick = typeof window.showOpenFilePicker === 'function'
const dirty = () => ed.value !== saved
function showName() {
  $('#name').textContent = name() + '.s'
  $('#name').classList.toggle('dirty', dirty())
  document.title = (dirty() ? '• ' : '') + name() + '.s'
}
function loaded(text, file) {
  store.set('name', file.replace(/\.s$/, ''))
  saved = text
  store.set('saved', text)
  setSource(text)
  showName()
}
async function open() {
  if (dirty() && !confirm(name() + '.s has unsaved changes. Open another file and drop them?')) return
  if (canPick) {
    try {
      const [h] = await showOpenFilePicker({ types: [{ description: 's', accept: { 'text/plain': ['.s'] } }] })
      handle = h
      loaded(await (await h.getFile()).text(), h.name)
    } catch {}
    return
  }
  $('#file').click()
}
$('#file').onchange = async () => {
  const f = $('#file').files[0]
  if (!f) return
  handle = null
  loaded(await f.text(), f.name)
  $('#file').value = ''
}
async function save() {
  const t = ed.value
  try {
    if (!handle && canPick) handle = await showSaveFilePicker({ suggestedName: name() + '.s', types: [{ description: 's', accept: { 'text/plain': ['.s'] } }] })
    if (handle) {
      const w = await handle.createWritable()
      await w.write(t)
      await w.close()
      store.set('name', handle.name.replace(/\.s$/, ''))
    } else download(name() + '.s', t, 'text/plain')
    saved = t
    store.set('saved', t)
    showName()
  } catch (e) { if (e.name !== 'AbortError') { status.className = 'err'; status.textContent = 'save: ' + e.message } }
}
$('#open').onclick = open
$('#save').onclick = save
$('#new').onclick = () => {
  if (dirty() && !confirm(name() + '.s has unsaved changes. Start a new program and drop them?')) return
  handle = null
  loaded('', 'page.s')
}
$('#s5').onclick = () => download(name() + '.s5', encode(ed.value), 'application/octet-stream')
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

/** The page compiled: HTML and CSS, and a script only when the page changes. */
async function compiled() {
  const f = document.createElement('iframe')
  f.style.display = 'none'
  f.srcdoc = '<!doctype html><html><head></head><body></body></html>'
  document.body.append(f)
  await new Promise((r) => (f.onload = r))
  try {
    const parts = await fetch('s-core.parts.json').then((r) => (r.ok ? r.json() : null))
    const core = parts ? '' : await (await fetch('s-core.js')).text()
    return await compilePage(ed.value, { doc: f.contentDocument, lib: LIB, core, parts })
  } finally { f.remove() }
}
$('#export').onclick = async () => {
  status.className = ''
  status.textContent = 'compiling…'
  let r
  try { r = await compiled() } catch (e) { status.className = 'err'; status.textContent = 'compile: ' + e.message; return }
  download(name() + '.html', r.html, 'text/html')
  const gz = await gzipSize(r.html)
  status.textContent = `compiled: ${kb(utf8(r.html))} (${kb(gz)} gzipped), ` + (r.dynamic ? `a script with ${r.units} pieces of code and ${r.words} library words` : 'no script') + (r.errors.length ? ` · ${r.errors.length} errors at build: ${r.errors[0]}` : '')
}
$('#exportReader').onclick = async () => {
  const lib = await (await fetch('s-web.min.js').then((r) => (r.ok ? r : fetch('s-web.js')))).text()
  const src = ed.value
  const title = (src.match(/\{([^{}]*)\}\s+title\b/) || [])[1] || name()
  const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>${esc(title)}</title>
</head>
<body>
<script type="text/s">
${src.replace(/<\/script/gi, '<\\/script')}
</script>
<script>
${lib.replace(/<\/script/gi, '<\\/script')}
</script>
</body>
</html>
`
  download(name() + '.reader.html', html, 'text/html')
  status.className = ''
  status.textContent = `with the reader: ${kb(utf8(html))} (${kb(await gzipSize(html))} gzipped)`
}
async function gzipSize(s) {
  if (typeof CompressionStream === 'undefined') return 0
  const st = new Blob([s]).stream().pipeThrough(new CompressionStream('gzip'))
  return (await new Response(st).arrayBuffer()).byteLength
}
$('#forget').onclick = () => { view.contentWindow.postMessage({ forget: true }, '*'); run() }
$('#run').onclick = run
$('#ref').onclick = () => { $('#words').hidden = !$('#words').hidden }
// ---------- the split ----------

$('#split').addEventListener('pointerdown', (e) => {
  const move = (ev) => { $('#left').style.width = Math.max(260, Math.min(innerWidth - 220, ev.clientX)) + 'px' }
  const up = () => { removeEventListener('pointermove', move); removeEventListener('pointerup', up); view.style.pointerEvents = ''; ed.paint() }
  view.style.pointerEvents = 'none'
  addEventListener('pointermove', move)
  addEventListener('pointerup', up)
  e.preventDefault()
})

// ---------- start ----------

;(async () => {
  let s = store.get('src', null)
  if (s == null) s = ''
  ed.value = s
  ed.focus()
  measure()
  showName()
  if (ready) run()
  else view.contentWindow.postMessage({ ping: true }, '*')
})()
window.ed = ed
