// node: the machine a compiled page carries, cut into parts with what each part names, for
// shake.js to put back together with only the parts a page reaches.
//
// A part is one of:
//   - a top-level declaration (function, class, const), named by what it declares;
//   - one method of Rt, named `.method`, reached through any `.method` in a reached part;
//   - one entry of a table (CMD, RESUME, api), kept when its key is wanted: `cmd:+`, `res:events`, `api:resume`;
//   - one statement inside attachDom or laneWords: a declaration as above, `word('x', …)` kept
//     when the word x is wanted (`word:x`), a statement after `// shake: key` kept when key is
//     wanted, and any other statement kept when everything it names is kept.
// Each part is minified on its own; names at the top of a part stay as they are, so the parts
// still find each other once joined.
import * as acorn from 'acorn'

const TABLES = { CMD: 'cmd', RESUME: 'res', api: 'api' }
const GROUPS = new Set(['attachDom', 'laneWords'])

/** The parts of src (a script). minify: async (code) => code. */
export async function cut(src, minify) {
  const comments = []
  const ast = acorn.parse(src, { ecmaVersion: 'latest', sourceType: 'script', onComment: comments })
  const keyBefore = (node) => {
    for (const c of comments) {
      const m = /^\s*shake:\s*(\S+)\s*$/.exec(c.value)
      if (m && c.end <= node.start && !src.slice(c.end, node.start).trim()) return m[1]
    }
    return null
  }
  const parts = []
  // `// shake: weak a b` inside a part: a and b are not reasons to keep anything; the code
  // reaches them only when another part already brought them. `// shake: uses a` is a reason.
  const notes = (node) => {
    const weak = new Set(), more = []
    for (const c of comments) {
      if (c.start < node.start || c.end > node.end) continue
      const m = /^\s*shake:\s*(weak|uses)\s+(.+)$/.exec(c.value)
      if (m) for (const n of m[2].trim().split(/\s+/)) (m[1] === 'weak' ? weak.add(n) : more.push(n))
    }
    return { weak, more }
  }
  const add = async (p, code, node) => {
    p.code = (await minify(code)).trim()
    if (node) {
      const { weak, more } = notes(node)
      p.uses = [...uses(node).filter((u) => !weak.has(u)), ...more]
    } else p.uses = []
    parts.push(p)
  }
  const body = (stmts, group) => Promise.all([]).then(async () => {
    for (const st of stmts) await statement(st, group)
  })
  async function statement(st, group) {
    const text = src.slice(st.start, st.end)
    const key = keyBefore(st)
    if (st.type === 'FunctionDeclaration' && GROUPS.has(st.id.name) && !group) {
      const head = src.slice(st.start, st.body.start) + '{'
      parts.push({ group: null, defines: [st.id.name], uses: [], head, code: null, opens: st.id.name })
      await body(st.body.body, st.id.name)
      return
    }
    if (st.type === 'ClassDeclaration' && st.id.name === 'Rt') {
      const ctor = st.body.body.find((m) => m.kind === 'constructor')
      await add({ group, defines: ['Rt'] }, `class Rt{${src.slice(ctor.start, ctor.end)}}`, ctor)
      for (const m of st.body.body) {
        if (m === ctor) continue
        const name = m.key.name ?? m.key.value
        await add({ group, defines: ['.' + name] }, `Rt.prototype[${JSON.stringify(name)}]=function${src.slice(m.value.start, m.value.end)}`, m.value)
      }
      return
    }
    if (st.type === 'VariableDeclaration' && st.declarations.length === 1 && TABLES[st.declarations[0].id.name] && st.declarations[0].init.type === 'ObjectExpression') {
      const name = st.declarations[0].id.name, tag = TABLES[name]
      await add({ group, defines: [name] }, `${st.kind} ${name}={}`, null)
      for (const p of st.declarations[0].init.properties) {
        const k = p.key.name ?? p.key.value
        const v = p.method ? 'function' + src.slice(p.value.start, p.value.end) : src.slice(p.value.start, p.value.end)
        await add({ group, defines: [], key: `${tag}:${k}` }, `${name}[${JSON.stringify(k)}]=${v}`, p.value)
      }
      return
    }
    const defines = declared(st)
    let k = key
    if (!k && st.type === 'ExpressionStatement' && st.expression.type === 'CallExpression' && st.expression.callee.type === 'Identifier' &&
      (st.expression.callee.name === 'word' || st.expression.callee.name === 'w') && st.expression.arguments[0]?.type === 'Literal') {
      k = 'word:' + st.expression.arguments[0].value
    }
    const p = { group, defines }
    if (k) p.key = k
    else if (st.type === 'ReturnStatement') p.always = true
    else if (!defines.length) p.follower = true
    if (st.type === 'ReturnStatement') { p.code = text; p.uses = uses(st); parts.push(p); return }
    await add(p, text, st)
  }
  await body(ast.body, null)
  return parts
}

function declared(st) {
  if (st.type === 'FunctionDeclaration' || st.type === 'ClassDeclaration') return [st.id.name]
  if (st.type === 'VariableDeclaration') return st.declarations.flatMap((d) => (d.id.type === 'Identifier' ? [d.id.name] : []))
  return []
}

/** The names a node uses: identifiers, and `.name` for each property read by name. */
function uses(node) {
  const out = new Set()
  const walk = (n, parent, field) => {
    if (!n || typeof n.type !== 'string') return
    if (n.type === 'Identifier') {
      const isProp = parent && ((parent.type === 'MemberExpression' && field === 'property' && !parent.computed) ||
        ((parent.type === 'Property' || parent.type === 'MethodDefinition' || parent.type === 'PropertyDefinition') && field === 'key' && !parent.computed))
      if (isProp && parent.type === 'MemberExpression') out.add('.' + n.name)
      else if (!isProp) out.add(n.name)
      if (parent && parent.type === 'Property' && parent.shorthand && field === 'key') out.add(n.name)
      return
    }
    for (const f in n) {
      if (f === 'type' || f === 'start' || f === 'end') continue
      const v = n[f]
      if (Array.isArray(v)) for (const x of v) walk(x, n, f)
      else if (v && typeof v.type === 'string') walk(v, n, f)
    }
  }
  walk(node, null, null)
  return [...out]
}
