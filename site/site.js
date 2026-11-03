// Every s block on the site: highlighted the way the machine reads its words, right-aligned
// as s is written. A block with class "ex" is an editor that runs on the machine in this page
// (machine.js, the s machine ported from rt.rs); one marked data-static shows what the Rust
// machine printed, for programs that need the shell, threads or the network.
(function () {
  const CMDS = new Set((window.S && S.CMD_WORDS) || [])
  const WORDS = new Set(['try', 'lanes', 'lane', 'endlane', 'collect', 'into', 'reduce', 'lanewise'])
  const SIGIL = '~`#;:@$.'
  const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  const isNum = (w) => /^-?(\d+(\.\d+)?|\.\d+)$/.test(w) || w === 'inf' || w === '-inf' || w === 'NaN'
  const ws = (c) => c === ' ' || c === '\n' || c === '\t' || c === '\r'

  function brace(src, i) {
    let d = 0, j = i
    for (; j < src.length; j++) {
      if (src[j] === '{') d++
      else if (src[j] === '}') { d--; if (d === 0) return j + 1 }
    }
    return src.length
  }
  function word(w) {
    if (/[⟪⟫¦⨝…]/.test(w) || w === '[[' || w === ']]' || w === '||' || w === '||>') return ['t-lane', esc(w)]
    if (CMDS.has(w) || WORDS.has(w)) return ['t-cmd', esc(w)]
    if (w.length > 1 && w[0] === '\\') return ['t-esc', esc(w)]
    if (isNum(w)) return ['t-num', esc(w)]
    const last = w[w.length - 1]
    if ((last === '?' || last === '!') && w.length > 1) {
      const [c, h] = word(w.slice(0, -1))
      return [c, h + '<span class="t-cond">' + last + '</span>']
    }
    if (w.length > 1 && SIGIL.includes(last)) {
      const local = w[0] === ':' && w.length > 2 ? ' t-local' : ''
      if (last === '~') return ['t-look' + local, esc(w)]
      if (last === '$') return ['t-def' + local, esc(w)]
      if (last === '.') return ['t-call' + local, esc(w)]
      return ['t-set' + local, esc(w)]
    }
    return ['', esc(w)]
  }
  function highlight(src) {
    const toks = []
    let i = 0
    while (i < src.length) {
      const c = src[i]
      if (ws(c)) { let j = i; while (j < src.length && ws(src[j])) j++; toks.push({ ws: src.slice(i, j) }); i = j; continue }
      if (c === '{') { const j = brace(src, i); toks.push({ brace: src.slice(i, j) }); i = j; continue }
      if (c === "'") { let j = src.indexOf("'", i + 1); j = j < 0 ? src.length : j + 1; toks.push({ cls: 't-txt', h: esc(src.slice(i, j)) }); i = j; continue }
      if (c === '⟨') {
        let j = src.indexOf('⟩', i); j = j < 0 ? src.length : j + 1
        while (j < src.length && !ws(src[j])) j++
        const t = src.slice(i, j), suf = t.slice(t.indexOf('⟩') + 1)
        toks.push({ cls: suf ? word('x' + suf)[0] : '', h: esc(t) }); i = j; continue
      }
      let j = i
      while (j < src.length && !ws(src[j]) && src[j] !== '{') j++
      const [cls, h] = word(src.slice(i, j))
      toks.push({ cls, h, w: src.slice(i, j) })
      i = j
    }
    // a brace followed on its line by // is a comment
    for (let k = 0; k < toks.length; k++) {
      if (toks[k].w !== '//') continue
      toks[k].cls = 't-com'
      let p = k - 1
      if (p >= 0 && toks[p].ws !== undefined && !toks[p].ws.includes('\n')) p--
      if (p >= 0 && toks[p].brace !== undefined) toks[p].com = true
    }
    return toks.map((t) => {
      if (t.ws !== undefined) return t.ws
      if (t.brace !== undefined) {
        if (t.com) return '<span class="t-com">' + esc(t.brace) + '</span>'
        // a brace is text until something runs it; its words are coloured as they would run
        const closed = t.brace.endsWith('}')
        const inner = t.brace.slice(1, closed ? -1 : undefined)
        return '<span class="t-txt">{</span>' + highlight(inner) + (closed ? '<span class="t-txt">}</span>' : '')
      }
      return t.cls ? '<span class="' + t.cls + '">' + t.h + '</span>' : t.h
    }).join('')
  }
  window.highlightS = highlight

  function run(src, stdin, argv) {
    let out = ''
    const q = stdin ? stdin.replace(/\n$/, '').split('\n') : []
    const rt = new S.Rt({ lib: S.LIB, vfs: new Map(), write: (s) => { out += s }, stdin: () => (q.length ? q.shift() : null) })
    rt.setArgv(argv)
    let st
    try { st = rt.runText(src, 50e6) } catch (e) { st = { error: String(e && e.message || e) } }
    return { out, st }
  }
  function show(pre, r) {
    let h = esc(r.out)
    const st = r.st
    if (st && st.error) h += (r.out ? '\n' : '') + '<span class="bad">s: ' + esc(st.error) + '</span>'
    else if (st && st.exit !== undefined) h += (r.out ? '\n' : '') + '<span class="end">exit ' + st.exit + '</span>'
    else if (st === 'ran') h += (r.out ? '\n' : '') + '<span class="end">stopped after 50 million steps</span>'
    else if (!r.out) h = '<span class="end">nothing printed</span>'
    pre.innerHTML = h
  }

  function editor(box) {
    const code = box.querySelector('pre.s')
    const src0 = code.textContent.replace(/^\n/, '').replace(/\s+$/, '')
    const stat = box.hasAttribute('data-static')
    const bar = document.createElement('div')
    bar.className = 'bar'
    bar.innerHTML = '<span class="lab"></span>'
    bar.querySelector('.lab').textContent = box.dataset.lab || 's'
    const ed = document.createElement('div')
    ed.className = 'ed'
    const inner = document.createElement('div')
    inner.style.cssText = 'position:relative;display:inline-block;min-width:100%;vertical-align:top'
    const pre = document.createElement('pre')
    pre.setAttribute('aria-hidden', 'true')
    inner.appendChild(pre)
    ed.appendChild(inner)
    let ta = null
    const paint = (s) => { pre.innerHTML = highlight(s) + (s.endsWith('\n') ? ' ' : '') }
    paint(src0)
    code.replaceWith(ed)
    box.insertBefore(bar, ed)
    const copy = document.createElement('button')
    copy.type = 'button'
    copy.textContent = 'Copy'
    copy.addEventListener('click', () => {
      const t = ta ? ta.value : src0
      const done = () => { copy.textContent = 'Copied'; setTimeout(() => (copy.textContent = 'Copy'), 1200) }
      try { navigator.clipboard.writeText(t).then(done, () => sel()) } catch (e) { sel() }
      function sel() { if (ta) { ta.focus(); ta.select() } }
    })
    if (stat) { bar.appendChild(copy); return }

    ta = document.createElement('textarea')
    ta.spellcheck = false
    ta.autocapitalize = 'off'
    ta.setAttribute('autocomplete', 'off')
    ta.setAttribute('aria-label', 's program')
    ta.id = box.id ? box.id + '-src' : ''
    ta.value = src0
    inner.appendChild(ta)
    let io = null, sin = null, sargs = null
    if (box.hasAttribute('data-stdin') || box.hasAttribute('data-args')) {
      io = document.createElement('div')
      io.className = 'io'
      if (box.hasAttribute('data-stdin')) {
        const l = document.createElement('label')
        l.innerHTML = '<span>stdin, one line per line</span>'
        sin = document.createElement('textarea')
        sin.rows = 3
        sin.spellcheck = false
        sin.id = (box.id || 'ex') + '-stdin'
        sin.value = box.dataset.stdin.replace(/\\n/g, '\n')
        l.appendChild(sin)
        io.appendChild(l)
      }
      if (box.hasAttribute('data-args')) {
        const l = document.createElement('label')
        l.innerHTML = '<span>arguments</span>'
        sargs = document.createElement('input')
        sargs.id = (box.id || 'ex') + '-args'
        sargs.value = box.dataset.args
        l.appendChild(sargs)
        io.appendChild(l)
      }
      box.insertBefore(io, ed.nextSibling)
    }
    const out = document.createElement('pre')
    out.className = 'out'
    out.dataset.lab = 'output'
    out.setAttribute('aria-live', 'polite')
    box.appendChild(out)
    const go = () => show(out, run(ta.value, sin ? sin.value : '', sargs ? sargs.value.split(/\s+/).filter(Boolean) : []))
    const runb = document.createElement('button')
    runb.type = 'button'
    runb.className = 'run'
    runb.textContent = 'Run'
    runb.title = 'Ctrl+Enter'
    runb.addEventListener('click', go)
    const reset = document.createElement('button')
    reset.type = 'button'
    reset.textContent = 'Reset'
    reset.addEventListener('click', () => { ta.value = src0; paint(src0); go() })
    bar.append(runb, reset, copy)
    ta.addEventListener('input', () => paint(ta.value))
    ta.addEventListener('focus', () => ed.classList.add('focus'))
    ta.addEventListener('blur', () => ed.classList.remove('focus'))
    ta.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); go(); return }
      if (e.key === 'Tab' && !e.shiftKey && !e.altKey) {
        e.preventDefault()
        if (!document.execCommand || !document.execCommand('insertText', false, '  ')) {
          const a = ta.selectionStart, b = ta.selectionEnd
          ta.setRangeText('  ', a, b, 'end')
          paint(ta.value)
        }
      }
    })
    go()
  }

  function init() {
    document.querySelectorAll('.ex').forEach(editor)
    document.querySelectorAll('pre.s:not(.ex pre)').forEach((p) => { p.innerHTML = highlight(p.textContent.replace(/^\n/, '').replace(/\s+$/, '')) })
    document.querySelectorAll('td.code, code.s').forEach((c) => { c.innerHTML = highlight(c.textContent) })
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init)
  else init()
})()
