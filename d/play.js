// The d playground: JS with $ on the left, the page it makes on the right.
const $q = (s) => document.querySelector(s)
const src = $q('#src'), view = $q('#view'), status = $q('#status')
const store = {
  get(k, d) { try { const v = localStorage.getItem('dplay.' + k); return v == null ? d : JSON.parse(v) } catch { return d } },
  set(k, v) { try { localStorage.setItem('dplay.' + k, JSON.stringify(v)) } catch {} },
}
let lib = '', timer = 0, handle = null, saved = store.get('saved', '')
const name = () => store.get('name', 'page')
const kb = (n) => (n < 1024 ? n + ' B' : (n / 1024).toFixed(1) + ' KB')

// ---------- running ----------

function page(code, forExport) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>${name()}</title>
</head>
<body>
<script>${lib.replace(/<\/script/gi, '<\\/script')}</script>
<script>
${forExport ? '' : `const size = () => ({ ok: 1, html: [...document.body.childNodes].filter((n) => n.nodeName !== 'SCRIPT' && !(n.nodeType === 3 && !n.data.trim())).map((n) => n.outerHTML ?? n.textContent).join('').length, els: document.body.querySelectorAll('*:not(script)').length })
`}${forExport ? `{ const r = (0, eval)(${JSON.stringify(code)}); if (r instanceof Node && !r.isConnected) document.body.append(r) }` : `try {
  const r = (0, eval)(${JSON.stringify(code)})
  if (r instanceof Node && !r.isConnected) document.body.append(r)
  parent.postMessage(size(), '*')
} catch (e) {
  const m = /<anonymous>:(\\d+)/.exec(e.stack || '')
  parent.postMessage({ err: e.message, line: m ? +m[1] : null }, '*')
}
new MutationObserver(() => parent.postMessage(size(), '*')).observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true })`}
</script>
</body>
</html>`
}
function run() {
  clearTimeout(timer)
  status.textContent = ''
  view.srcdoc = page(src.value, false)
}
let htmlLen = 0, els = 0
addEventListener('message', (e) => {
  if (e.source !== view.contentWindow) return
  const d = e.data || {}
  if (d.err) {
    status.textContent = '× ' + d.err + (d.line ? ' · line ' + d.line : '')
    status.onclick = d.line ? () => goLine(d.line) : null
  }
  if (d.ok) { htmlLen = d.html; els = d.els; count() }
})
function goLine(n) {
  const lines = src.value.split('\n')
  const i = lines.slice(0, n - 1).join('\n').length + (n > 1 ? 1 : 0)
  src.focus()
  src.setSelectionRange(i, i + (lines[n - 1] || '').length)
}
function count() {
  const t = src.value
  const abbr = [...t.matchAll(/\$`((?:[^`\\]|\\.)*)`/g)].reduce((a, m) => a + m[1].length, 0)
  $q('#count').textContent = `${t.length} characters · ${abbr} in $\`…\` · page: ${els} elements, ${htmlLen} characters of HTML` + (abbr ? ` · ×${(htmlLen / abbr).toFixed(1)} per abbreviation character` : '')
}

// ---------- editing ----------

const dirty = () => src.value !== saved
function showName() {
  $q('#name').textContent = name() + '.js'
  $q('#name').classList.toggle('dirty', dirty())
  document.title = (dirty() ? '• ' : '') + name() + '.js'
}
src.addEventListener('input', () => {
  store.set('src', src.value)
  showName()
  count()
  if ($q('#live').checked) { clearTimeout(timer); timer = setTimeout(run, 400) }
})
src.addEventListener('keydown', (e) => {
  if (e.key === 'Tab') { e.preventDefault(); src.setRangeText('  ', src.selectionStart, src.selectionEnd, 'end'); src.dispatchEvent(new Event('input')) }
  if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
    const ls = src.value.lastIndexOf('\n', src.selectionStart - 1) + 1
    const ind = /^[ \t]*/.exec(src.value.slice(ls))[0]
    if (ind) { e.preventDefault(); src.setRangeText('\n' + ind, src.selectionStart, src.selectionEnd, 'end'); src.dispatchEvent(new Event('input')) }
  }
})
addEventListener('keydown', (e) => {
  const ctrl = e.ctrlKey || e.metaKey
  if (ctrl && e.key === 'Enter') { e.preventDefault(); run() }
  else if (ctrl && e.key.toLowerCase() === 's') { e.preventDefault(); save() }
  else if (ctrl && e.key.toLowerCase() === 'o') { e.preventDefault(); open() }
  else if (e.key === 'F1') { e.preventDefault(); $q('#words').hidden = !$q('#words').hidden }
  else if (e.key === 'Escape') $q('#words').hidden = true
})

// ---------- files ----------

const canPick = typeof window.showOpenFilePicker === 'function'
const types = [{ description: 'JavaScript', accept: { 'text/javascript': ['.js'] } }]
function download(file, data, type) {
  const a = document.createElement('a')
  a.href = URL.createObjectURL(new Blob([data], { type }))
  a.download = file
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 1000)
}
function loaded(text, file) {
  store.set('name', file.replace(/\.js$/, ''))
  saved = text
  store.set('saved', text)
  src.value = text
  store.set('src', text)
  showName(); count(); run()
}
async function open() {
  if (dirty() && !confirm(name() + '.js has unsaved changes. Open another file and drop them?')) return
  if (canPick) {
    try { const [h] = await showOpenFilePicker({ types }); handle = h; loaded(await (await h.getFile()).text(), h.name) } catch {}
    return
  }
  $q('#file').click()
}
$q('#file').onchange = async () => {
  const f = $q('#file').files[0]
  if (!f) return
  handle = null
  loaded(await f.text(), f.name)
  $q('#file').value = ''
}
async function save() {
  try {
    if (!handle && canPick) handle = await showSaveFilePicker({ suggestedName: name() + '.js', types })
    if (handle) {
      const w = await handle.createWritable()
      await w.write(src.value)
      await w.close()
      store.set('name', handle.name.replace(/\.js$/, ''))
    } else download(name() + '.js', src.value, 'text/javascript')
    saved = src.value
    store.set('saved', saved)
    showName()
  } catch (e) { if (e.name !== 'AbortError') status.textContent = 'save: ' + e.message }
}
$q('#open').onclick = open
$q('#save').onclick = save
$q('#new').onclick = () => { if (dirty() && !confirm(name() + '.js has unsaved changes. Start a new page and drop them?')) return; handle = null; loaded('', 'page.js') }
$q('#export').onclick = () => download(name() + '.html', page(src.value, true), 'text/html')
$q('#run').onclick = run
$q('#ref').onclick = () => { $q('#words').hidden = !$q('#words').hidden }
$q('#split').addEventListener('pointerdown', (e) => {
  const move = (ev) => { $q('#left').style.width = Math.max(260, Math.min(innerWidth - 220, ev.clientX)) + 'px' }
  const up = () => { removeEventListener('pointermove', move); removeEventListener('pointerup', up); view.style.pointerEvents = '' }
  view.style.pointerEvents = 'none'
  addEventListener('pointermove', move)
  addEventListener('pointerup', up)
  e.preventDefault()
})

;(async () => {
  lib = await (await fetch('d.min.js')).text()
  src.value = store.get('src', '')
  showName(); count(); run()
  src.focus()
})()
window.src = src
