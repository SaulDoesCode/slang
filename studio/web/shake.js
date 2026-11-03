// The machine a compiled page carries, put together from its parts (parts.mjs) with only the
// parts the page reaches: from the roots, every part a kept part names, every table entry
// whose key is wanted, and every statement whose names are all kept.

/** parts: from s-core.parts.json. roots: names ('Rt', '.run'). keys: wanted keys ('cmd:+'). */
export function shake(parts, roots, keys) {
  const byName = new Map()
  parts.forEach((p, i) => { for (const d of p.defines) byName.set(d, i) })
  const keep = new Set()
  const todo = []
  const need = (name) => { const i = byName.get(name); if (i !== undefined && !keep.has(i)) { keep.add(i); todo.push(i) } }
  const take = (i) => { if (!keep.has(i)) { keep.add(i); todo.push(i) } }
  for (const r of roots) need(r)
  parts.forEach((p, i) => { if (p.always || (p.key && keys.has(p.key))) take(i) })
  for (;;) {
    while (todo.length) {
      const p = parts[todo.pop()]
      if (p.group) need(p.group)
      for (const u of p.uses) need(u)
    }
    // statements that only tie kept parts together
    let more = false
    parts.forEach((p, i) => {
      if (!p.follower || keep.has(i) || (p.group && !keep.has(byName.get(p.group)))) return
      if (p.uses.every((u) => !byName.has(u) || keep.has(byName.get(u)))) { take(i); more = true }
    })
    if (!more) break
  }
  let out = ''
  let open = null
  parts.forEach((p, i) => {
    if (!keep.has(i)) return
    if (open && p.group !== open) { out += '}\n'; open = null }
    if (p.opens) { out += p.head; open = p.opens; return }
    out += p.code + '\n'
  })
  if (open) out += '}\n'
  return { code: out, kept: keep.size, of: parts.length }
}
