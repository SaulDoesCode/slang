// s5 (BEADS.md §1): text as 5-bit codes. encode(text) → Uint8Array, decode(bytes) → text.

const FIGS = '0123456789.,;:!?\'"-(){}\n\t/~@#='
const SYMS = '<>+*&$%[]\\|^_`'
const CAP = 27, FIG = 28, SYM = 29, UNI = 30, BEAD = 31
const BEADS = '⟪¦⟫⨝'
const isLower = (c) => c >= 'a' && c <= 'z'
const isUpper = (c) => c >= 'A' && c <= 'Z'
const isLetter = (c) => isLower(c) || isUpper(c)

/** The codes of a text. Figure lock is entered before a run of three or more figures; `#`,
 *  whose figure code follows a single FIG as the code that enters lock, is written in lock. */
export function codes(text) {
  const cs = Array.from(text)
  const out = []
  let lock = false
  const uni = (c) => {
    const v = c.codePointAt(0)
    let n = 1
    while (v >= 2 ** (5 * n)) n++
    out.push(UNI, n)
    for (let i = n - 1; i >= 0; i--) out.push((v >> (5 * i)) & 31)
  }
  for (let i = 0; i < cs.length; i++) {
    const c = cs[i]
    const f = FIGS.indexOf(c)
    if (lock) {
      if (f >= 0) { out.push(f); continue }
      out.push(31); lock = false
    }
    if (f >= 0) {
      let run = 0
      while (i + run < cs.length && FIGS.includes(cs[i + run])) run++
      if (run >= 3 || c === '#') { out.push(FIG, FIG); lock = true; out.push(f); continue }
      out.push(FIG, f)
      continue
    }
    if (c === ' ') { out.push(0); continue }
    if (isLower(c)) { out.push(c.charCodeAt(0) - 96); continue }
    if (isUpper(c)) {
      // a token of capitals takes one CAP CAP
      let j = i
      while (j < cs.length && isLetter(cs[j])) j++
      const word = cs.slice(i, j)
      if (word.length > 1 && word.every(isUpper)) {
        out.push(CAP, CAP)
        for (const w of word) out.push(w.toLowerCase().charCodeAt(0) - 96)
        i = j - 1
        continue
      }
      out.push(CAP, c.toLowerCase().charCodeAt(0) - 96)
      continue
    }
    const y = SYMS.indexOf(c)
    if (y >= 0) { out.push(SYM, y); continue }
    const b = BEADS.indexOf(c)
    if (b >= 0) { out.push(BEAD, b); continue }
    uni(c)
  }
  return out
}

export function encode(text) {
  const cs = codes(text)
  const head = []
  let n = cs.length
  do { let b = n & 127; n = Math.floor(n / 128); if (n) b |= 128; head.push(b) } while (n)
  const body = new Uint8Array(Math.ceil(cs.length * 5 / 8))
  let bit = 0
  for (const c of cs) {
    for (let k = 4; k >= 0; k--) {
      if ((c >> k) & 1) body[bit >> 3] |= 128 >> (bit & 7)
      bit++
    }
  }
  const out = new Uint8Array(head.length + body.length)
  out.set(head); out.set(body, head.length)
  return out
}

export function decode(bytes) {
  let p = 0, n = 0, shift = 0
  for (;;) { const b = bytes[p++]; n += (b & 127) * 2 ** shift; shift += 7; if (!(b & 128)) break }
  const cs = []
  let bit = p * 8
  for (let i = 0; i < n; i++) {
    let c = 0
    for (let k = 0; k < 5; k++) { c = (c << 1) | ((bytes[bit >> 3] >> (7 - (bit & 7))) & 1); bit++ }
    cs.push(c)
  }
  let out = '', i = 0, lock = false, cap = 0
  const letter = (c) => { const l = String.fromCharCode(96 + c); out += cap ? l.toUpperCase() : l; if (cap === 1) cap = 0 }
  const readUni = () => { const m = cs[i++]; let v = 0; for (let k = 0; k < m; k++) v = v * 32 + cs[i++]; out += String.fromCodePoint(v) }
  while (i < cs.length) {
    const c = cs[i++]
    if (lock) {
      if (c === 31) lock = false
      else if (c === 30) readUni()
      else out += FIGS[c]
      continue
    }
    if (cap === 2 && !(c >= 1 && c <= 26)) cap = 0
    if (c === 0) out += ' '
    else if (c <= 26) letter(c)
    else if (c === CAP) { if (cs[i] === CAP) { i++; cap = 2 } else cap = 1 }
    else if (c === FIG) { const d = cs[i++]; if (d === FIG) lock = true; else out += FIGS[d] }
    else if (c === SYM) out += SYMS[cs[i++]]
    else if (c === UNI) readUni()
    else if (c === BEAD) {
      const m = cs[i++]
      if (m === 4) { let l = ''; while (i < cs.length && cs[i] !== 0) l += String.fromCharCode(96 + cs[i++]); i++; out += '⟪' + l + ':' }
      else out += BEADS[m]
    }
  }
  return out
}
