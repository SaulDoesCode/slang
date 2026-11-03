// The compiler: a program runs once at build time, in a page of its own, and what it built is
// written out as HTML and CSS. When the page has events, timers, inputs, pages or kept names,
// the code those run is turned into JavaScript functions and sent with a snapshot of the names
// and the part of the machine the functions call. The reader, the program's top level and the
// library words nothing reaches stay behind.
import { Rt, parse, text } from './s.js'
import { attachDom, elementSpec, attrWord, eventWord, selWord } from './dom.js'
import { shake } from './shake.js'

const enc = (v) => {
  if (typeof v === 'number') return Number.isFinite(v) ? v : { n: String(v) }
  if (typeof v === 'string') return v
  return { w: v.w, k: v.k }
}

/** Builds the page of src in doc (a fresh document with a window) and returns { html, js, out, errors }. */
export async function compilePage(src, opts) {
  const { doc, lib, core } = opts
  const out = [], errors = []
  const rt = new Rt({ lib, write: (s) => out.push(s) })
  rt.setArgv([])
  doc.body.replaceChildren()
  const dom = attachDom(rt, { document: doc, mount: doc.body, storage: null, onError: (e) => errors.push(e.msg) })
  dom.run(src)
  const D = dom.D
  const timers = D.timers.map((t) => [t.code, t.ms, t.repeat])
  dom.stop()
  const title = doc.title
  const sheet = doc.getElementById('s-sheet')
  const css = sheet ? sheet.textContent : ''

  const live = (x) => x.el && x.el.isConnected
  const events = D.events.filter(live), regions = D.regions.filter(live), attrs = D.attrs.filter(live), twoWay = D.twoWay.filter(live)
  const routers = D.routers.filter((r) => r.isConnected)
  const dynamic = events.length || timers.length || twoWay.length || D.kept.size || (routers.length && D.pages.size > 1)

  if (!dynamic) return { html: page(title, css, doc.body.innerHTML, ''), js: '', out, errors, dynamic: false }

  // elements the script needs to find
  const ids = new Map()
  const id = (el) => {
    if (!ids.has(el)) { ids.set(el, ids.size); el.setAttribute('data-s', String(ids.size - 1)) }
    return ids.get(el)
  }
  const R = {
    events: events.map((e) => [id(e.el), e.ev, e.code]),
    regions: regions.map((r) => [id(r.el), r.code, r.parts, [...r.reads], r.c ? 1 : 0]),
    attrs: attrs.map((a) => [id(a.el), a.name, a.parts, [...a.reads]]),
    twoWay: twoWay.map((b) => [id(b.el), b.k]),
    routers: routers.map(id),
    pages: [...D.pages],
    rules: [...D.rules],
    kept: [...D.kept],
    timers,
  }
  const body = doc.body.innerHTML

  // ---------- what the page can reach ----------
  const units = new Map() // text → unit
  const queue = []
  const want = (t) => { if (typeof t === 'string' && t.length < 20000 && !units.has(t)) queue.push(t) }
  const keys = new Set(), files = new Map()
  const cmds = new Set()   // the commands the page's code names
  let host = false         // whether the code makes elements, attributes or events as it runs
  const reachKey = (k) => {
    if (keys.has(k) || !rt.repl.has(k)) return
    keys.add(k)
    for (const m of rt.repl.get(k)) if (m.code && m.code.u) want(m.code.u.src)
  }
  for (const e of R.events) want(e[2])
  for (const r of R.regions) if (r[1] !== null) want(r[1])
  for (const p of R.pages) want(p[1])
  for (const t of R.timers) want(t[0])
  for (const sc of rt.scripts) if (sc && sc.u) want(sc.u.src)
  for (const v of rt.mem) if (typeof v === 'string' && v.length < 2000) want(v)
  for (const v of rt.mem) if (v && typeof v === 'object') reachKey(v.w)
  const scan = (u) => {
    for (const op of u.ops) scanOp(op)
  }
  const scanOp = (op) => {
    switch (op[0]) {
      case 'push':
        if (typeof op[1] === 'string') { want(op[1]); if (op[1].includes('|')) want(op[1].replace(/\|/g, '~')) }
        else if (typeof op[1] === 'object') reachKey(op[1].w)
        break
      case 'word':
        reachKey(op[1])
        if (elementSpec(op[1]) || attrWord(op[1]) || eventWord(op[1]) || selWord(op[1])) host = true
        break
      case 'cmd': case 'native': reachKey(op[1]); cmds.add(op[1]); break
      case 'lookup': {
        const v = rt.mem[op[1]]
        if (typeof v === 'string' && !/\s/.test(v)) reachKey(v)
        break
      }
      case 'call': case 'callpass': {
        const name = rt.it.name(op[1])
        const t = rt.lib[name.replace(/^\.\//, '')]
        if (t !== undefined && !files.has(name)) { files.set(name, t); want(t) }
        break
      }
      case 'cond': scanOp(op[2]); break
      case 'lanes': for (const l of op[1].lanes) scan(l); break
    }
  }
  while (queue.length) {
    const t = queue.pop()
    if (units.has(t)) continue
    let u
    try { u = parse(t, rt.it) } catch { units.set(t, null); continue }
    units.set(t, u)
    scan(u)
  }
  // a file the page calls has its own words: all of them come along
  for (const [, t] of files) for (const [k, layers] of rt.repl) if (layers.some((m) => m.code && m.code.u && t.includes(m.code.u.src))) reachKey(k)
  while (queue.length) {
    const t = queue.pop()
    if (units.has(t)) continue
    let u
    try { u = parse(t, rt.it) } catch { units.set(t, null); continue }
    units.set(t, u)
    scan(u)
  }

  // ---------- code ----------
  const K = []
  const kIndex = new Map()
  const konst = (v) => {
    const key = typeof v === 'string' ? 's' + v : JSON.stringify(v)
    if (!kIndex.has(key)) { kIndex.set(key, K.length); K.push(v) }
    return `K[${kIndex.get(key)}]`
  }
  const fns = []
  const lanesK = []
  const unitJs = (u) => {
    const body = u.ops.map((op, i) => opJs(op, i === u.ops.length - 1)).join('\n')
    return `(R,d)=>{\n${body}\n}`
  }
  const lit = (v) => (typeof v === 'number' && Number.isFinite(v) ? String(v) : konst(enc(v)))
  const opJs = (op, tail) => {
    switch (op[0]) {
      case 'push': return typeof op[1] === 'object' ? `R.args.push(${konst(enc(op[1]))})` : typeof op[1] === 'number' && !Number.isFinite(op[1]) ? `R.args.push(${op[1] > 0 ? 'Infinity' : op[1] < 0 ? '-Infinity' : 'NaN'})` : `R.args.push(${lit(op[1])})`
      case 'interp': return `R.interpOp(${konst(op[1])})`
      case 'word': return `R.wordRc(${JSON.stringify(op[1])},${op[2]},${op[3] === null ? 'null' : JSON.stringify(op[3])});R.st(d)`
      case 'cmd': return `if(R.cmdSite(${JSON.stringify(op[1])}))return 1;R.st(d)`
      case 'native': return `if(R.cmd(${JSON.stringify(op[1])}))return 1;R.st(d)`
      case 'pop': return `R.popTo(${op[1]})`
      case 'def': return `R.defScript(${op[1]})`
      case 'setmem': return `R.setMem(${op[1]})`
      case 'setnum': return `R.setNum(${op[1]})`
      case 'inc': return `R.incBy(${op[1]},1)`
      case 'dec': return `R.incBy(${op[1]},-1)`
      case 'lookup': return `if(R.lookup(${op[1]}))return 1;R.st(d)`
      case 'call': return tail
        ? `{const k=R.l(${op[1]});if(R.args.length===1){R.repeat(k);R.st(d);return}return{tail:k}}`
        : `R.callSite(${op[1]});R.st(d)`
      case 'callpass': return tail ? `return{tail:R.l(${op[1]})}` : `R.call(R.l(${op[1]}));R.st(d)`
      case 'cond': return `if(R.ow===${op[1]}){${opJs(op[2], tail)}}`
      case 'lanes': {
        const i = lanesK.length
        lanesK.push(`{simd:${op[1].simd},lanes:[${op[1].lanes.map((l) => `{fn:${unitJs(l)},unit:-1}`).join(',')}]}`)
        return `R.lanesOp(L[${i}]);R.st(d)`
      }
    }
    throw new Error('compile: no form for ' + op[0])
  }
  const table = []
  for (const [t, u] of units) {
    if (!u) continue
    table.push(`[${konst(t)},{fn:${unitJs(u)},src:${konst(t)},unit:-1}]`)
  }

  // ---------- the names ----------
  const sparse = (a, f = enc) => { const o = []; a.forEach((v, i) => { if (v !== undefined) o.push([i, f(v)]) }); return o }
  const repl = []
  for (const [k, layers] of rt.repl) {
    // the machine registers its own words as it starts; a word the page reaches comes with every layer
    if (!keys.has(k)) continue
    const keep = layers
    if (!keep.length) continue
    repl.push([k, keep.map((m) => [m.nat ? null : enc(m.val), m.code && m.code.u ? m.code.u.src : null, m.gen, m.lib ? 1 : 0, m.nat || null])])
  }
  const S = {
    names: rt.it.names, local: rt.it.local.map((x) => (x ? 1 : 0)),
    mem: sparse(rt.mem), nums: sparse(rt.nums), lists: sparse(rt.lists, (l) => l.map(enc)),
    scripts: sparse(rt.scripts, (s) => [s.u.src, s.b]),
    repl, gen: rt.gen, replNum: rt.replNum, ow: rt.ow, files: [...files],
  }
  // code that can run a text made as the page runs needs the machine's one-word reader
  const interp = /\b(?:R|rt)\.(?:lookup|dispatch)\b/.test(table.join('')) || cmds.has('~') || cmds.has('?)')
  // code that runs a text it may have built as it ran (~, ?), ><) carries the reader too
  const reads = !!opts.parts && (cmds.has('~') || cmds.has('?)') || cmds.has('><'))
  const js = `(()=>{
const {Rt,attachDom,classify,parse}=sCore()
const K=${JSON.stringify(K)}
const L=[${lanesK.join(',\n')}]
const C=new Map([${table.join(',\n')}])
const S=${JSON.stringify(S)},P=${JSON.stringify(R)}
const dec=(v)=>v&&typeof v==='object'?('n' in v?Number(v.n.replace('inf','Infinity')):v):v
const rt=new Rt({lib:Object.fromEntries(S.files),write:(s)=>console.log(s.replace(/^\\n/,''))})
rt.reader=(src)=>{const u=C.get(src);if(u)return u;${interp ? `const w=src.trim();if(w&&!/[\\s{}'⟨⟪¦⟫⨝]/.test(w))return{ops:[classify(w,rt.it)],spans:[0],src,unit:-1};` : ''}${reads ? 'return parse(src,rt.it);' : ''}throw new Error('this text was made while the page ran, and a compiled page has no reader: '+src.slice(0,60))}
const dom=attachDom(rt,{mount:document.body})
S.names.forEach((n,i)=>{rt.it.sym(n);rt.it.local[i]=!!S.local[i]})
rt.mem=[];rt.nums=[];rt.lists=[];rt.scripts=[]
for(const[i,v]of S.mem)rt.mem[i]=dec(v)
for(const[i,v]of S.nums)rt.nums[i]=dec(v)
for(const[i,l]of S.lists)rt.lists[i]=l.map(dec)
for(const[i,[t,b]]of S.scripts)rt.scripts[i]={u:rt.codeOf(t),b}
for(const[k,ls]of S.repl)rt.repl.set(k,ls.filter((m)=>!m[4]||rt.natives.has(m[4])).map(([v,t,gen,lib,nat])=>nat?{val:nat,code:{n:rt.natives.get(nat)},gen,nat}:{val:dec(v),code:t===null?null:{u:rt.codeOf(t)},gen,lib:!!lib}))
rt.gen=S.gen;rt.replNum=S.replNum;rt.ow=S.ow
dom.resume(P)
})()`
  // the machine: with its parts, only the ones this page reaches
  let machine = core, kept = null
  if (opts.parts) {
    const roots = new Set(['Rt', 'attachDom'])
    if (interp) roots.add('classify').add('.interpret')
    if (reads) roots.add('parse').add('.interpret').add('.lanesOp')
    if (files.size) roots.add('.fileText').add('.fileFrame')
    if (table.some((t) => t.includes('{tail:'))) roots.add('.tailInto')
    // the e. names an event sets, when the page's code reads them
    if ([...units.keys()].some((t) => /\be\.(value|checked|key|id|text)/.test(t))) roots.add('setEventNames')
    for (const m of js.matchAll(/\b(?:R|rt)\.(\w+)/g)) roots.add('.' + m[1])
    const want = new Set(['api:resume'])
    for (const k of ['events', 'regions', 'attrs', 'twoWay', 'pages', 'rules', 'kept', 'timers', 'routers']) if (R[k].length) want.add('res:' + k)
    for (const c of cmds) want.add('cmd:' + c)
    for (const k of keys) want.add('word:' + k)
    if (host) want.add('host')
    // text read as the page runs may name any command or word
    if (reads) for (const p of opts.parts) if (p.key && /^(cmd|word):/.test(p.key)) want.add(p.key)
    if (reads) want.add('host')
    const sh = shake(opts.parts, roots, want)
    machine = `var sCore=function(){\n${sh.code}return{Rt,attachDom${interp ? ',classify' : ''}${reads ? ',parse' : ''}}};\n`
    kept = [sh.kept, sh.of]
  }
  const html = page(title, css, body, `<script>\n${(machine + '\n' + js).replace(/<\/(script)/gi, '<\\/$1')}\n</script>`)
  return { html, js: machine + js, out, errors, dynamic: true, units: units.size, words: keys.size, parts: kept }
}

const escHtml = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
function page(title, css, body, script) {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>${escHtml(title || '')}</title>
${css ? `<style id="s-sheet">${css.replace(/<\/style/gi, '<\\/style')}\n</style>\n` : ''}</head>
<body>
${body}
${script}
</body>
</html>
`
}
