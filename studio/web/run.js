// Running s in a page: a program describes the page and the page is built from it.
// Scripts of type text/s run when the page loads, into the element they name with
// data-mount, or into the body.
import { Rt } from './s.js'
import { LIB } from './std.js'
import { attachDom } from './dom.js'

export function runS(src, opts = {}) {
  const doc = opts.document || document
  const rt = new Rt({ lib: LIB, write: opts.write || ((s) => console.log(s.replace(/^\n/, ''))) })
  rt.setArgv([])
  if (opts.onJoin) rt.onJoin = opts.onJoin
  const dom = attachDom(rt, { document: doc, mount: opts.mount || doc.body, onError: opts.onError, storage: opts.storage })
  const st = dom.run(src)
  return { rt, dom, status: st }
}

/** Where an error happened, as a character offset in the program text. */
export function errorAt(src, e) {
  if (!e || !e.code) return null
  const span = e.code.spans[e.pc] ?? 0
  const sub = Array.from(e.code.src)
  const all = Array.from(src)
  if (e.code.src === src) return span
  const i = src.indexOf(e.code.src)
  if (i < 0) return null
  return Array.from(src.slice(0, i)).length + span
}

export function autorun(doc = document) {
  for (const s of doc.querySelectorAll('script[type="text/s"]')) {
    const mount = s.dataset.mount ? doc.querySelector(s.dataset.mount) : doc.body
    runS(s.textContent.replace(/<\\\/script/gi, '</script'), { document: doc, mount })
  }
}
