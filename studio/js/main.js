// s³ studio: the world as bricks, plates, spheres and zones, edited through the server.
import * as THREE from '../vendor/three.module.js'
import * as T from './text.js'
import { initWindows, dropConsole, resetLayout } from './ui.js'
import { newPlayer, stepPlayer, eyeHeight, hits as playerHits, P as WALK, scaleBody } from './walk.js'
// a brick is about 40 cm to the walker: code stands at bench height, devices are board parts
const BODY = 2.5
scaleBody(BODY)
import * as D from './datum.js'
import { datumLayer, gimbal } from './datumview.js'

const $ = (s) => document.querySelector(s)
const h = (tag, props = {}, ...kids) => {
  const e = document.createElement(tag)
  for (const k in props) {
    const v = props[k]
    if (v == null || v === false) continue
    if (k === 'class') e.className = v
    else if (k === 'text') e.textContent = v
    else if (k.startsWith('on')) e.addEventListener(k.slice(2), v)
    else if (k === 'value') e.value = v
    else e.setAttribute(k, v === true ? '' : v)
  }
  for (const c of kids.flat()) if (c != null && c !== false) e.append(c)
  return e
}
const store = {
  get(k, d) { try { const v = localStorage.getItem('s3.' + k); return v == null ? d : JSON.parse(v) } catch { return d } },
  set(k, v) { try { localStorage.setItem('s3.' + k, JSON.stringify(v)) } catch {} },
}

// ---------- state ----------

const S = {
  name: store.get('world', 'scratch'),
  W: { spaces: new Map(), plates: new Map(), bricks: new Map(), spheres: new Map(), links: new Map(), zones: new Map(), datums: new Map(), tracks: new Map(), goal: null, params: {} },
  selDatum: null,
  gimbal: false,
  space: 0,
  sel: new Set(),
  selSphere: null,
  selSpheres: new Set(),
  gridPlace: store.get('gridPlace', false),
  wireFrom: null,
  selZone: null,
  selPlate: null,
  hover: null,
  tool: null,
  armed: null,
  run: null,
  rt: null,
  breaks: new Set(),
  errBricks: new Set(),
  cur: null,
  palette: null,
  groupOf: new Map(),
  undo: 0, redo: 0,
  layers: { shelves: true, links: true, zones: true, labels: true, minimap: true, ...store.get('layers', {}) },
  found: new Set(),
}

async function api(cmd, o = {}) {
  const r = await fetch('api', { method: 'POST', body: JSON.stringify({ cmd, name: S.name, ...o }) })
  return r.json()
}

function status(t, err = false) {
  const s = $('#status')
  s.textContent = t
  s.classList.toggle('err', err)
}

function log(text, cls = '') {
  const out = $('#out')
  const e = h('div', { class: cls, text })
  out.append(e)
  while (out.childNodes.length > 4000) out.firstChild.remove()
  out.scrollTop = out.scrollHeight
  return e
}

// ---------- the mirror ----------

const TABLES = { space: 'spaces', plate: 'plates', brick: 'bricks', sphere: 'spheres', link: 'links', zone: 'zones', datum: 'datums', track: 'tracks' }

function loadWorld(j) {
  const W = S.W
  for (const k of Object.values(TABLES)) W[k].clear()
  for (const [key, t] of [['spaces', 'space'], ['plates', 'plate'], ['bricks', 'brick'], ['spheres', 'sphere'], ['links', 'link'], ['zones', 'zone'], ['datums', 'datum'], ['tracks', 'track']]) {
    for (const r of j[key] || []) W[TABLES[t]].set(r.id, r)
  }
  W.goal = j.goal || null
  W.params = j.params || {}
  W.name = j.name
  dirty()
}

function applyChanges(c) {
  if (!c) return
  if (c.error) { status(c.error, true); return false }
  resolved.clear()
  for (const id of c.del || []) for (const k of Object.values(TABLES)) S.W[k].delete(id)
  for (const r of c.put || []) S.W[TABLES[r.t]].set(r.id, r)
  if ('undo' in c) { S.undo = c.undo; S.redo = c.redo }
  syncUndo()
  for (const id of [...S.sel]) if (!S.W.bricks.has(id)) S.sel.delete(id)
  if (S.selSphere && !S.W.spheres.has(S.selSphere)) S.selSphere = null
  if (S.selDatum && !S.W.datums.has(S.selDatum)) S.selDatum = null
  dirty()
  return true
}

async function edit(e, quiet = false) {
  const r = await api('edit', { edit: e })
  if (r.error) { status(r.error, true); return null }
  applyChanges(r)
  if (!liveAfter(r) && !quiet) status('saved')
  refreshInspector()
  tutorialEvent(e.op)
  return r.result || {}
}

// a running world takes edits live, at each zone's next event; some edits end the run
function liveAfter(r) {
  if (!S.run) return false
  if (r.stopped) { endRunHere(r.stopped); return true }
  if (r.live) {
    status('live: the edit applies at the next event')
    if (S.run.done) api('poll', { ops: 1 }).then((rep) => { if (S.run) showReport(rep) })
    return true
  }
  return false
}

function syncUndo() {
  $('#b-undo').disabled = !S.undo
  $('#b-redo').disabled = !S.redo
}

// ---------- scene ----------

const canvas = $('#view')
const renderer = new THREE.WebGLRenderer({ canvas, antialias: true })
renderer.setPixelRatio(Math.min(devicePixelRatio || 1, 2))
const BG = 0xe9dbc6
const scene = new THREE.Scene()
scene.background = new THREE.Color(BG)
scene.fog = new THREE.Fog(BG, 70, 220)
const camera = new THREE.PerspectiveCamera(60, 1, 0.05, 1000)
T.shared.uTanHalf.value = Math.tan(THREE.MathUtils.degToRad(30))
scene.add(new THREE.HemisphereLight(0xfffaf2, 0xd8c09c, 2.2))
const sun = new THREE.DirectionalLight(0xffffff, 1.1)
sun.position.set(8, 16, 10)
scene.add(sun)

const groups = { space: new THREE.Group(), graph: new THREE.Group(), zones: new THREE.Group(), over: new THREE.Group() }
const datums = datumLayer(THREE)
const gizmo = gimbal(THREE)
for (const g of Object.values(groups)) scene.add(g)
scene.add(datums.group, gizmo.group)

const KCOL = {
  lit: '#ead6b4', num: '#a8d8cc', esc: '#cfc8bd', name: '#bcd0ec', read: '#cad7ea', box: '#e6ecd9', quote: '#ecd099',
  lib: '#d3c6ec', geo: '#eab0bd', sock: '#d6e6c6', raw: '#dcd6cc',
  io: '#ecb98f', file: '#e1ae82', cond: '#e0cb83', list: '#cdbbe6', code: '#eaafa6', kern: '#afcbe9', thread: '#bcdaaf', numc: '#e9c876',
}
const SKCOL = { src: '#d98c3f', fn: '#5b8fb9', ratchet: '#b5546b', out: '#6e9c5e', gate: '#7d8794', mast: '#8a7a64', port: '#c9a227', asm: '#6a5d8c' }
const PCOL = { main: '#d6c09d', script: '#c5d4b2', shelf: '#d5bfd4', inside: '#cbc2ae' }
const ZCOL = { async: '#5b8fb9', thread: '#7a5532', net: '#4f7a3a', rpc: '#b5546b', scope: '#a0782c' }
const DCOL = { s: '#8a7a64', logic: '#3b8f6a', lisp: '#7b3f8c', forth: '#8a4b2a', apl: '#3b5f9a', elec: '#c9a227', machine: '#b5546b' }
const DIALECTS = ['logic', 's']

function brickColor(b) {
  if (b.kind === 'cmd') {
    const g = S.groupOf.get(b.word) || 'code'
    return KCOL[g === 'num' ? 'numc' : g] || KCOL.code
  }
  return KCOL[b.kind] || KCOL.lit
}

// the text of a brick as s: what typing it again would make
function brickText(b) {
  const w = b.word
  switch (b.kind) {
    case 'num': case 'cmd': return w + (b.cond || '')
    case 'esc': return '\\' + w + (b.cond || '')
    case 'quote': return "'" + w + "'"
    case 'read': return '{~' + w + '~}'
    case 'sock': return '~' + w + '~'
    case 'raw': return w
    case 'box': return w ? '{' + w + '}' : '{' + spaceText(b.inner) + '}' + (b.cond || '')
    case 'name': return w + (b.cap || '') + (b.cond || '')
    default: return w + (b.cond || '')
  }
}

// the lines a brick shows on its faces: its word whole, and for a box the code inside it
// as s, so a box reads like the braces it stands for instead of hiding them
// the whole line a brick stands in, the brick marked: what pointing at one word reads
function columnLine(b) {
  const col = columnOf(b)
  return col.map((x) => {
    const t = x.kind === 'box' && !x.word ? brickLines(x).join(' ') : brickText(x)
    return x.id === b.id && col.length > 1 ? '[' + t + ']' : t
  }).join(' ')
}
function brickLines(b) {
  if (b.kind !== 'box' || b.word) return [brickText(b)]
  const ls = spaceText(b.inner).split('\n').filter((l) => l.trim())
  if (!ls.length) return ['{}' + (b.cond || '')]
  ls[0] = '{' + ls[0]
  ls[ls.length - 1] += '}' + (b.cond || '')
  return ls
}
// words laid on one face, left aligned as code is, fitted to the face; curve text stays
// sharp however small it gets, so a long box is read by coming close
// long lines break at spaces toward a square block, so a sentence in a box stays legible
function wrapSquare(lines) {
  const total = lines.reduce((a, l) => a + T.measure(l) * T.LINE, 0)
  const target = Math.max(3, Math.sqrt(total * 1.6))
  const out = []
  for (const l of lines) {
    if (T.measure(l) <= target * 1.15) { out.push(l); continue }
    let cur = ''
    for (const w of l.split(' ')) {
      const t = cur ? cur + ' ' + w : w
      if (cur && T.measure(t) > target) { out.push(cur); cur = w } else cur = t
    }
    if (cur) out.push(cur)
  }
  return out
}
function faceItems(lines, c, r, u, items, room = 0.88, most = 0.5) {
  lines = wrapSquare(lines)
  const ws = lines.map((l) => T.measure(l))
  const wmax = Math.max(0.01, ...ws)
  const size = Math.min(most, room / wmax, room / (lines.length * T.LINE))
  const top = (lines.length * T.LINE) / 2
  lines.forEach((l, i) => {
    const dx = (ws[i] - wmax) / 2 * size, dy = (top - (i + 0.5) * T.LINE) * size
    items.push({ text: l, size, c: [c[0] + r[0] * dx + u[0] * dy, c[1] + r[1] * dx + u[1] * dy, c[2] + r[2] * dx + u[2] * dy], r, u })
  })
}

// columns and text, mirrored for display only; the server decides what runs
function platesIn(space) {
  return [...S.W.plates.values()].filter((p) => p.space === space).sort((a, b) => a.at[1] - b.at[1] || a.at[0] - b.at[0] || a.at[2] - b.at[2] || a.id - b.id)
}
function plateUnder(space, c) {
  let best = null
  for (const p of S.W.plates.values()) {
    if (p.space !== space) continue
    if (c[0] >= p.at[0] && c[0] < p.at[0] + p.w && c[2] >= p.at[2] && c[2] < p.at[2] + p.d && c[1] > p.at[1]) {
      if (!best || p.at[1] > best.at[1] || (p.at[1] === best.at[1] && p.id > best.id)) best = p
    }
  }
  return best
}
const cellKey = (s, c) => s + ':' + c[0] + ',' + c[1] + ',' + c[2]
let cellIndex = new Map()
function reindex() {
  cellIndex = new Map()
  for (const b of S.W.bricks.values()) cellIndex.set(cellKey(b.space, b.cell), b)
}
const brickAt = (s, c) => cellIndex.get(cellKey(s, c))
function columnOf(b) {
  const col = []
  let y = b.cell[1]
  while (brickAt(b.space, [b.cell[0], y - 1, b.cell[2]])) y--
  let x
  while ((x = brickAt(b.space, [b.cell[0], y, b.cell[2]]))) { col.push(x); y++ }
  return col
}
function arrowKey(p, x, z) {
  switch (p.arrow) {
    case '-x': return [-z, -x]
    case '+z': return [-x, z]
    case '-z': return [x, -z]
    default: return [z, x]
  }
}
function plateColumns(p) {
  const bs = [...S.W.bricks.values()].filter((b) => b.space === p.space && plateUnder(p.space, b.cell) === p)
  const by = new Map()
  for (const b of bs) {
    const k = b.cell[0] + ',' + b.cell[2]
    if (!by.has(k)) by.set(k, [])
    by.get(k).push(b)
  }
  const cols = []
  for (const v of by.values()) {
    v.sort((a, b) => a.cell[1] - b.cell[1])
    let cur = []
    for (const b of v) {
      if (cur.length && b.cell[1] !== cur[cur.length - 1].cell[1] + 1) { cols.push(cur); cur = [] }
      cur.push(b)
    }
    if (cur.length) cols.push(cur)
  }
  cols.sort((a, b) => {
    const ka = arrowKey(p, a[0].cell[0], a[0].cell[2]), kb = arrowKey(p, b[0].cell[0], b[0].cell[2])
    return ka[0] - kb[0] || ka[1] - kb[1] || a[0].cell[1] - b[0].cell[1]
  })
  return cols
}
function spaceText(space, depth = 0) {
  if (depth > 32) return '…'
  return platesIn(space).map((p) => plateColumns(p).map((c) => c.map((b) => brickText(b)).join(' ')).join('\n')).join('\n')
}

// ---------- building the view ----------

let needBuild = true
function dirty() { needBuild = true }

const boxGeo = new THREE.BoxGeometry(0.94, 0.94, 0.94)
const brickMat = new THREE.MeshLambertMaterial({ color: 0xffffff })
const faceMat = T.faceMaterial('#2e2216')
const sphereGeo = new THREE.SphereGeometry(1, 32, 22)
const coneGeo = new THREE.ConeGeometry(1, 1, 12)
let brickMesh = null, brickIds = [], faceMesh = null
const plateMeshes = [], sphereMeshes = []
const disposeGroup = (g) => {
  for (const o of [...g.children]) {
    g.remove(o)
    if (o === gridMesh) continue
    o.traverse((x) => { if (x.geometry && ![boxGeo, sphereGeo, coneGeo, devGeo, portGeo, poleGeo].includes(x.geometry)) x.geometry.dispose(); if (x.material && x.material !== brickMat && x.material !== faceMat && !x.material.userData.shared) x.material.dispose() })
  }
}

function build() {
  needBuild = false
  reindex()
  if (typeof walk !== 'undefined' && walk.on) reindexDevices()
  disposeGroup(groups.space)
  disposeGroup(groups.graph)
  disposeGroup(groups.zones)
  plateMeshes.length = 0
  sphereMeshes.length = 0
  brickMesh = null
  faceMesh = null
  const space = S.space
  // plates
  for (const p of platesIn(space)) {
    if (!S.layers.shelves && p.kind === 'shelf') continue
    const g = new THREE.Mesh(new THREE.BoxGeometry(p.w, 0.12, p.d), new THREE.MeshStandardMaterial({ color: PCOL[p.kind] || PCOL.main, roughness: 0.9 }))
    g.position.set(p.at[0] + p.w / 2, p.at[1] + 0.94, p.at[2] + p.d / 2)
    g.userData = { plate: p.id }
    if (S.selPlate === p.id) g.material.emissive = new THREE.Color(0x3a2a18)
    groups.space.add(g)
    plateMeshes.push(g)
    const pts = []
    for (let x = 0; x <= p.w; x++) pts.push(p.at[0] + x, p.at[1] + 1.002, p.at[2], p.at[0] + x, p.at[1] + 1.002, p.at[2] + p.d)
    for (let z = 0; z <= p.d; z++) pts.push(p.at[0], p.at[1] + 1.002, p.at[2] + z, p.at[0] + p.w, p.at[1] + 1.002, p.at[2] + z)
    const lg = new THREE.BufferGeometry()
    lg.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3))
    groups.space.add(new THREE.LineSegments(lg, new THREE.LineBasicMaterial({ color: TH().plateLine, transparent: true, opacity: 0.6 })))
    // the arrow: reading direction
    const dir = { '+x': [1, 0, 0], '-x': [-1, 0, 0], '+z': [0, 0, 1], '-z': [0, 0, -1] }[p.arrow]
    const cone = new THREE.Mesh(coneGeo, new THREE.MeshBasicMaterial({ color: TH().accent }))
    const start = { '+x': [p.at[0] - 0.6, p.at[2] + 0.5], '-x': [p.at[0] + p.w + 0.6, p.at[2] + p.d - 0.5], '+z': [p.at[0] + p.w - 0.5, p.at[2] - 0.6], '-z': [p.at[0] + 0.5, p.at[2] + p.d + 0.6] }[p.arrow]
    cone.position.set(start[0], p.at[1] + 1, start[1])
    cone.scale.set(0.22, 0.6, 0.22)
    cone.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), new THREE.Vector3(...dir))
    groups.space.add(cone)
    const lab = T.makeLabel(p.kind + (p.name && p.kind !== 'main' ? ' ' + p.name : ''), TH().ink2, 0.32)
    lab.position.set(p.at[0] + 0.2, p.at[1] + 1.35, p.at[2] - 0.4)
    groups.space.add(lab)
  }
  // bricks
  const hidden = (b) => !S.layers.shelves && (plateUnder(space, b.cell) || {}).kind === 'shelf'
  const bs = [...S.W.bricks.values()].filter((b) => b.space === space && !hidden(b))
  brickIds = bs.map((b) => b.id)
  if (bs.length) {
    brickMesh = new THREE.InstancedMesh(boxGeo, brickMat, bs.length)
    const m = new THREE.Matrix4(), col = new THREE.Color()
    bs.forEach((b, i) => {
      m.makeTranslation(b.cell[0] + 0.5, b.cell[1] + 0.5, b.cell[2] + 0.5)
      brickMesh.setMatrixAt(i, m)
      col.set(brickColor(b))
      if (S.errBricks.has(b.id)) col.set('#e08a80')
      if (S.found.has(b.id)) col.set('#f2d37a')
      if (S.sel.has(b.id)) col.offsetHSL(0, 0.1, -0.16)
      if (S.cur === b.id) col.set('#f0a040')
      brickMesh.setColorAt(i, col)
    })
    groups.space.add(brickMesh)
    // words on the four side faces, skipping faces another brick covers
    const items = []
    const faces = [[[0, 0, 1], [1, 0, 0]], [[0, 0, -1], [-1, 0, 0]], [[1, 0, 0], [0, 0, -1]], [[-1, 0, 0], [0, 0, 1]]]
    for (const b of bs) {
      const ls = brickLines(b)
      for (const [n, r] of faces) {
        if (brickAt(space, [b.cell[0] + n[0], b.cell[1], b.cell[2] + n[2]])) continue
        faceItems(ls, [b.cell[0] + 0.5 + n[0] * 0.476, b.cell[1] + 0.5, b.cell[2] + 0.5 + n[2] * 0.476], r, [0, 1, 0], items)
      }
      // a box shows its code on top too, read from above along the plate
      if (ls.length > 1 && !brickAt(space, [b.cell[0], b.cell[1] + 1, b.cell[2]])) faceItems(ls, [b.cell[0] + 0.5, b.cell[1] + 0.952, b.cell[2] + 0.5], [1, 0, 0], [0, 0, -1], items)
      if (S.breaks.has(b.id)) {
        const dot = new THREE.Mesh(sphereGeo, new THREE.MeshBasicMaterial({ color: 0xb0413e }))
        dot.position.set(b.cell[0] + 0.5, b.cell[1] + 1.02, b.cell[2] + 0.5)
        dot.scale.setScalar(0.12)
        groups.space.add(dot)
      }
      if (b.cap && b.kind === 'name') {
        const cap = T.makeLabel(b.cap + (b.cond || ''), TH().accent, 0.3)
        cap.position.set(b.cell[0] + 0.5, b.cell[1] + 1.12, b.cell[2] + 0.5)
        if (!brickAt(space, [b.cell[0], b.cell[1] + 1, b.cell[2]])) groups.space.add(cap)
      }
    }
    faceMesh = new THREE.Mesh(T.faceGeometry(items), faceMat)
    groups.space.add(faceMesh)
  }
  buildGraph()
  buildTracks()
  buildDatums()
  if (space === 0 && S.layers.zones) buildZones()
  renderCrumbs()
}

// ---------- tracks, items and machines ----------

const MACHINES = ['conveyor', 'rotator', 'splitter', 'merger', 'cutter', 'stacker', 'painter', 'reader', 'writer', 'bin']
const MACHINE_TEXT = { conveyor: 'passes items on', rotator: 'turns a cube a quarter', splitter: 'hands out by turns', merger: 'takes from every track ending at it', cutter: 'a cube into three byte bars', stacker: 'three bars into a cube', painter: 'value and "and" | "or"', reader: 'an item onto its wires', writer: 'what its wires bring onto a track', bin: 'takes everything' }
const MCOL = { conveyor: '#8a7a64', rotator: '#5b8fb9', splitter: '#a0782c', merger: '#a0782c', cutter: '#b5546b', stacker: '#6a5d8c', painter: '#c9a227', reader: '#4f7a3a', writer: '#d98c3f', bin: '#4a4040' }
// a device whose body is one machine word works as that machine
function machineWord(sp) {
  const t = spaceText(sp.body).trim()
  return MACHINES.includes(t) ? t : null
}
function machineLoad(id) {
  const m = S.rt && S.rt.machines.get(id)
  if (!m) return ''
  const n = m.queue.length + m.hold.length
  return n ? n + ' held' : ''
}
function trackText(id) {
  const t = S.W.tracks.get(id)
  if (!t) return ''
  const items = S.rt && S.rt.tracks.get(id)
  return 'track ' + t.name + '  ·  ' + t.cells.length + ' cells at ' + t.speed + ' a tick' + (items ? '  ·  ' + items.length + ' riding' : '')
}
const beltGeo = new THREE.BoxGeometry(0.9, 0.12, 0.9)
const chevGeo = new THREE.ConeGeometry(0.2, 0.34, 3)
const trackMeshes = []
function buildTracks() {
  trackMeshes.length = 0
  const m = new THREE.Matrix4(), q = new THREE.Quaternion(), up = new THREE.Vector3(0, 1, 0)
  for (const t of S.W.tracks.values()) {
    if ((t.space || 0) !== S.space) continue
    const belt = new THREE.InstancedMesh(beltGeo, new THREE.MeshStandardMaterial({ color: theme === 'monokai' ? '#4a3c4a' : '#3d3530', roughness: 0.8 }), t.cells.length)
    const chev = new THREE.InstancedMesh(chevGeo, new THREE.MeshBasicMaterial({ color: TH().accent }), t.cells.length)
    t.cells.forEach((c, i) => {
      m.makeTranslation(c[0] + 0.5, c[1] + 0.06, c[2] + 0.5)
      belt.setMatrixAt(i, m)
      const n = t.cells[i + 1] || (i ? [2 * c[0] - t.cells[i - 1][0], 2 * c[1] - t.cells[i - 1][1], 2 * c[2] - t.cells[i - 1][2]] : [c[0] + 1, c[1], c[2]])
      const d = new THREE.Vector3(n[0] - c[0], n[1] - c[1], n[2] - c[2]).normalize()
      q.setFromUnitVectors(up, d)
      m.compose(new THREE.Vector3(c[0] + 0.5, c[1] + 0.14, c[2] + 0.5), q, new THREE.Vector3(1, 1, 0.25))
      chev.setMatrixAt(i, m)
    })
    belt.userData = { track: t.id }
    groups.space.add(belt, chev)
    trackMeshes.push(belt)
    if (S.layers.labels) {
      const c = t.cells[0]
      const lab = T.makeLabel(t.name, TH().ink2, 0.24)
      lab.position.set(c[0] + 0.5, c[1] + 0.5, c[2] + 0.5)
      groups.space.add(lab)
    }
  }
}
// items ride between ticks: each report gives cells, the view eases from where it was
const itemGroup = new THREE.Group()
scene.add(itemGroup)
const itemViews = new Map()
const cubeGeo = new THREE.BoxGeometry(0.46, 0.46, 0.46), barGeo = new THREE.BoxGeometry(0.62, 0.16, 0.16)
const CH = ['#d9534f', '#4caf50', '#4a7bd0']
let itemsAt = 0
function itemPoint(t, pos) {
  const i = Math.max(0, Math.min(t.cells.length - 1, Math.floor(pos))), j = Math.min(t.cells.length - 1, i + 1), f = pos - i
  const a = t.cells[i], b = t.cells[j]
  return new THREE.Vector3(a[0] + 0.5 + (b[0] - a[0]) * f, a[1] + 0.12 + (b[1] - a[1]) * f, a[2] + 0.5 + (b[2] - a[2]) * f)
}
function itemMesh(it) {
  let mesh
  if (it.k === 'cube') {
    const c = new THREE.Color(((it.v >> 16) & 255) / 255, ((it.v >> 8) & 255) / 255, (it.v & 255) / 255)
    mesh = new THREE.Mesh(cubeGeo, new THREE.MeshStandardMaterial({ color: c.convertSRGBToLinear(), roughness: 0.5 }))
    mesh.position.y = 0.23
  } else if (it.k === 'bar') {
    const c = new THREE.Color(CH[it.ch]).multiplyScalar(0.25 + 0.75 * it.v / 255)
    mesh = new THREE.Mesh(barGeo, new THREE.MeshStandardMaterial({ color: c, roughness: 0.5 }))
    mesh.position.y = 0.08
  } else {
    mesh = T.makeLabel(String(it.t).slice(0, 16), TH().ink, 0.3)
    mesh.position.y = 0.3
  }
  const g = new THREE.Group()
  g.add(mesh)
  if (it.k !== 'val' && S.layers.labels) {
    const lab = T.makeLabel(String(it.v), TH().ink2, 0.18)
    lab.position.y = it.k === 'cube' ? 0.62 : 0.32
    g.add(lab)
  }
  return g
}
function itemsArrived() {
  const seen = new Set()
  const tracks = S.rt ? S.rt.tracks : new Map()
  for (const [tid, items] of tracks) {
    const t = S.W.tracks.get(tid)
    if (!t || (t.space || 0) !== S.space) continue
    for (const it of items) {
      seen.add(it.id)
      let v = itemViews.get(it.id)
      if (!v) {
        v = { obj: itemMesh(it), track: tid, from: it.pos, to: it.pos }
        itemGroup.add(v.obj)
        itemViews.set(it.id, v)
      } else {
        v.from = v.track === tid ? v.shown ?? v.to : it.pos
        v.track = tid
        v.to = it.pos
      }
    }
  }
  for (const [id, v] of itemViews) if (!seen.has(id)) { itemGroup.remove(v.obj); itemViews.delete(id) }
  itemsAt = performance.now()
}
function updateItems(now) {
  if (!itemViews.size) return
  const f = Math.min(1, (now - itemsAt) / 100)
  for (const v of itemViews.values()) {
    const t = S.W.tracks.get(v.track)
    if (!t) continue
    v.shown = v.from + (v.to - v.from) * f
    v.obj.position.copy(itemPoint(t, v.shown))
  }
}
function clearItems() {
  for (const v of itemViews.values()) itemGroup.remove(v.obj)
  itemViews.clear()
}
// a track goes from the first cell clicked to the second, along x, then z
function trackClick(cell, keep) {
  if (!S.trackFrom) { S.trackFrom = cell; status('track from ' + cell[0] + ' ' + cell[2] + ': click where it ends'); return }
  const a = S.trackFrom
  S.trackFrom = null
  edit({ op: 'track', from: a, to: [cell[0], a[1], cell[2]], space: S.space })
  if (!keep && !walk.on) setTool(null)
}
function placeMachine(word, cell) {
  edit({ op: 'sphere', kind: 'fn', cell, code: word, reach: 0.01, space: S.space })
}

function sphereView(id) {
  const s = S.W.spheres.get(id)
  if (!s) return undefined
  const r = S.rt && S.rt.spheres.get(id)
  return r ? { ...s, pos: r.pos, aim: r.aim, reach: r.reach, hit: r.hit, shown: r.shown, error: r.error, notch: r.notch } : s
}

// ---------- datum cubes ----------

// a datum as the scene shows it: the record, with what a running program left in its names
function datumView(id) {
  const d = S.W.datums.get(id)
  if (!d) return undefined
  const r = S.rt && S.rt.datums && S.rt.datums.get(id)
  const prev = S.datumPreview && S.datumPreview.id === id ? S.datumPreview.parts : null
  const parts = { ...D.partsOf(d, r), ...(prev || {}) }
  let cell = d.cell
  if (walk.on && walk.carryDatum === id) { const hp = holdPoint(); cell = [hp.x - 0.5, hp.y - 0.5, hp.z - 0.5] }
  return { ...d, cell, value: r ? r.value : d.value, parts }
}
function datumItems() {
  const out = []
  for (const d of S.W.datums.values()) if ((d.space || 0) === S.space) out.push(datumView(d.id))
  return out
}
let datumLod = { near: [], far: [] }
function buildDatums() {
  const r = datumLod = datums.build(datumItems(), cam.pos.toArray(), walk.on ? null : S.selDatum)
  // seeing a toggled cube from afar is a tutorial step
  if (S.touched && r.far.includes(S.touched)) { S.touched = null; tutorialEvent('far') }
  if (S.selDatum && S.gimbal && !walk.on) {
    const d = datumView(S.selDatum)
    if (d) gizmo.show(d.cell, d.parts)
    else gizmo.hide()
  } else gizmo.hide()
}
let datumCells = new Set()
function datumAt(space, c) {
  for (const d of S.W.datums.values()) if ((d.space || 0) === space && d.cell[0] === c[0] && d.cell[1] === c[1] && d.cell[2] === c[2]) return d
  return null
}
// the cube nearest along a ray, with the tile it lands on
function nearestDatum(o, dir) {
  let best = null
  for (const d of datumItems()) {
    if (d.id === walk.carryDatum) continue
    const h = D.rayHit(d.cell, d.parts, o, dir)
    if (h && (!best || h.t < best.t)) best = { ...h, id: d.id }
  }
  return best
}
function datumPick(h) {
  const d = datumView(h.id)
  const n = D.dirToWorld(d.parts, h.normal)
  const pt = new THREE.Vector3(...h.point)
  const cell = [Math.floor(pt.x + n[0] * 0.5), Math.floor(pt.y + n[1] * 0.5), Math.floor(pt.z + n[2] * 0.5)]
  return { datum: h.id, face: h.face, u: h.u, v: h.v, bit: D.tileBit(h.face, h.u, h.v), point: pt, cell, dist: h.t }
}
function toggleTile(p) {
  S.touched = p.datum
  return edit({ op: 'tile', id: p.datum, face: p.face, u: p.u, v: p.v }, true).then((r) => {
    const d = S.W.datums.get(p.datum)
    if (r && d) status(d.name + ' bit ' + p.bit + ' → ' + ((d.value >> p.bit) & 1) + '  ·  ' + D.colorOf(d.value) + '  ·  ' + d.value)
    return r
  })
}
function datumText(d) {
  return 'datum ' + d.name + '  ·  ' + D.colorOf(d.value) + '  ·  ' + d.value + (D.schemaOf(d.parts).length ? '  ·  ' + D.schemaOf(d.parts).map((k) => k + ' ' + +d.parts[k].toFixed(4)).join(' ') : '')
}

// the gimbal: drag a knob or a square to scale, a ring to turn; Ctrl snaps, digits type the amount
function toScreen(v) {
  const p = v.clone().project(camera)
  const r = canvas.getBoundingClientRect()
  return [r.left + ((p.x + 1) / 2) * r.width, r.top + ((1 - p.y) / 2) * r.height]
}
function gimbalDown(e) {
  if (walk.on || !S.selDatum || !gizmo.where()) return false
  const r = canvas.getBoundingClientRect()
  ndc.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1)
  ray.setFromCamera(ndc, camera)
  const g = gizmo.hit(ray)
  if (!g) return false
  const d = datumView(S.selDatum)
  drag = { kind: 'gimbal', handle: g, base: g, id: d.id, start: { ...d.parts }, at: gizmo.where(), x0: e.clientX, y0: e.clientY, x: e.clientX, y: e.clientY, typed: '', snap: e.ctrlKey }
  canvas.setPointerCapture(e.pointerId)
  gimbalApply(drag)
  return true
}
function gimbalAmount(dr) {
  const at = dr.at, cs = toScreen(at.c), h = dr.handle
  if (h[0] === 'r') {
    const i = 'xyz'.indexOf(h[1])
    const axis = new THREE.Vector3(i === 0 ? 1 : 0, i === 1 ? 1 : 0, i === 2 ? 1 : 0)
    const sign = axis.dot(camera.position.clone().sub(at.c)) >= 0 ? 1 : -1
    return -D.sweep(cs, [dr.x0, dr.y0], [dr.x, dr.y]) * sign
  }
  if (h === 's') return Math.max(0.01, 1 + (dr.x - dr.x0 - (dr.y - dr.y0)) / 160)
  const keys = D.HANDLES[h]
  let dir = [0, 0], unit = 0
  for (const k of keys) {
    const i = 'xyz'.indexOf(k[1])
    const tip = toScreen(at.c.clone().add(at.ax[i]))
    const v = [tip[0] - cs[0], tip[1] - cs[1]]
    const len = Math.hypot(v[0], v[1]) || 1
    dir = [dir[0] + v[0] / len, dir[1] + v[1] / len]
    unit += len
  }
  unit /= keys.length
  const dl = Math.hypot(dir[0], dir[1]) || 1
  const px = ((dr.x - dr.x0) * dir[0] + (dr.y - dr.y0) * dir[1]) / dl
  // a knob moves half the growth of a centered axis, all of it on y, which grows from the floor
  return D.dragFactor(px * (keys.length === 1 && keys[0] === 'sy' ? 1 : 2), unit, dr.start[keys[0]])
}
function gimbalApply(dr) {
  dr.parts = D.gimbalParts(dr.handle, dr.start, gimbalAmount(dr), { snap: dr.snap, typed: dr.typed })
  S.datumPreview = { id: dr.id, parts: dr.parts }
  gizmo.active(dr.handle)
  buildDatums()
  status(Object.entries(dr.parts).map(([k, x]) => k + ' ' + +x.toFixed(4)).join('  ') + (dr.typed ? '  ·  typed ' + dr.typed : '') + (dr.snap ? '  ·  snapped' : '') + '  ·  release or Enter sets it, Esc cancels')
}
function gimbalMove(e) {
  drag.x = e.clientX
  drag.y = e.clientY
  drag.snap = e.ctrlKey
  gimbalApply(drag)
}
function gimbalCommit(dr) {
  S.datumPreview = null
  gizmo.active(null)
  const d = S.W.datums.get(dr.id)
  const changed = dr.parts && d && Object.entries(dr.parts).some(([k, x]) => x !== D.partsOf(d)[k])
  if (changed) edit({ op: 'transform', id: dr.id, parts: dr.parts })
  else dirty()
}
function gimbalKey(e) {
  e.preventDefault()
  const dr = drag
  const k = e.key
  if (k === 'Escape') { drag = null; S.datumPreview = null; gizmo.active(null); dirty(); status('gimbal cancelled'); return }
  if (k === 'Enter') { drag = null; gimbalCommit(dr); return }
  if (/^[xyzXYZ]$/.test(k)) { dr.handle = D.constrain(dr.base, k); dr.x0 = dr.x; dr.y0 = dr.y; dr.start = { ...dr.start, ...dr.parts } }
  else if (/^[0-9.\-]$/.test(k)) dr.typed += k
  else if (k === 'Backspace') dr.typed = dr.typed.slice(0, -1)
  else if (k === 'Control') dr.snap = true
  gimbalApply(dr)
}

function toggleGimbal() {
  S.gimbal = !S.gimbal
  for (const el of document.querySelectorAll('.tile[data-kind="gimbal"]')) el.classList.toggle('on', S.gimbal)
  status(S.gimbal ? 'gimbal: select a cube, drag a knob or a square to scale it, a ring to turn it · T puts it away' : 'gimbal put away')
  dirty()
}

function datumInspector(d) {
  if (!d) return
  const rec = S.W.datums.get(d.id)
  $('#ins-title').textContent = 'datum ' + d.name
  const name = h('input', { value: d.name, onchange: () => edit({ op: 'put', t: 'datum', ...rec, name: name.value.trim() }) })
  const setValue = (v) => edit({ op: 'value', id: d.id, value: Math.max(0, Math.min(0xffffff, Math.round(v))) })
  const color = h('input', { type: 'color', value: D.colorOf(rec.value), onchange: () => setValue(parseInt(color.value.slice(1), 16)) })
  const num = h('input', { value: rec.value, onchange: () => setValue(+num.value) })
  insBody.append(field('name', name), field('value', h('div', { class: 'row' }, color, num)))
  if (S.rt && d.value !== rec.value) insBody.append(h('div', { class: 'note', text: 'running: code left ' + d.value + ' (' + D.colorOf(d.value) + ')' }))
  // the 24 tiles, face by face: click one to toggle its bit
  insBody.append(h('b', { text: 'Tiles' }))
  const grid = h('div', { class: 'tiles' })
  const NIB = ['red high', 'red low', 'green high', 'green low', 'blue high', 'blue low']
  for (let f = 0; f < 6; f++) {
    const face = h('div', { class: 'face', title: D.FACE_NAMES[f] + ' · ' + NIB[f] })
    for (let v = 0; v < 2; v++) for (let u = 0; u < 2; u++) {
      const bit = D.tileBit(f, u, v)
      face.append(h('span', { class: 'bit' + ((rec.value >> bit) & 1 ? ' on' : ''), title: 'bit ' + bit, onclick: () => { S.touched = d.id; edit({ op: 'tile', id: d.id, face: f, u, v }) } }))
    }
    grid.append(h('div', { class: 'fcol' }, face, h('span', { class: 'fl', text: D.FACE_NAMES[f] })))
  }
  insBody.append(grid)
  // the transform parts: the schema beside the value
  insBody.append(h('b', { text: 'Schema' }))
  const parts = D.partsOf(rec)
  const row = h('div', { class: 'parts' })
  for (const k of D.PART_NAMES) {
    const inp = h('input', { value: +parts[k].toFixed(4), title: k, onchange: () => { const x = +inp.value; if (isFinite(x)) edit({ op: 'transform', id: d.id, parts: { [k]: x } }) } })
    row.append(h('label', {}, h('span', { text: k }), inp))
  }
  insBody.append(row)
  const present = D.schemaOf(parts)
  insBody.append(h('div', { class: 'note', text: 'schema: ' + [d.name, ...present.map((k) => d.name + '/' + k)].join(' ') }), h('div', { class: 'note', text: 'In s, ' + d.name + '~ is the value, ' + d.name + '/sx~ a part, and ' + d.name + ' schema puts the present parts on the line. A part at its default is not stored.' }))
  // the mutation log: s that redoes what the player did
  insBody.append(h('b', { text: 'Log' }))
  const ta = h('textarea', { class: 'code', spellcheck: 'false', readonly: true })
  ta.value = (rec.log || []).join('\n')
  insBody.append(ta, h('div', { class: 'row' },
    h('button', { class: 'btn', text: 'Copy', onclick: () => { navigator.clipboard && navigator.clipboard.writeText(ta.value).catch(() => {}); status('log copied as s text') } }),
    h('button', { class: 'btn', text: 'Fold', title: 'replace the log by the shortest program that rebuilds the cube', onclick: () => edit({ op: 'clearlog', id: d.id }) }),
    h('button', { class: 'btn', text: 'Turn (R)', onclick: () => edit({ op: 'rotate', ids: [d.id], pivot: rec.cell, turns: 1 }) }),
    h('button', { class: 'btn', text: 'Remove', onclick: () => { edit({ op: 'remove', ids: [d.id] }); S.selDatum = null } })))
  insBody.append(h('div', { class: 'note', text: 'cell ' + rec.cell.join(' ') + ' · the gimbal scales along the cube\'s axes and turns it about x y z' }))
}

const devGeo = new THREE.BoxGeometry(0.84, 0.84, 0.84)
const portGeo = new THREE.OctahedronGeometry(1)
const poleGeo = new THREE.CylinderGeometry(0.07, 0.1, 1, 8)
const inSpace = (sp) => (sp.space || 0) === S.space

function buildGraph() {
  const up = new THREE.Vector3(0, 1, 0)
  for (const s0 of S.W.spheres.values()) {
    if (!inSpace(s0)) continue
    const s = sphereView(s0.id)
    const g = new THREE.Group()
    const sel = S.selSphere === s.id || S.selSpheres.has(s.id)
    const lit = S.rt && s.kind === 'out' && s.shown === '1'
    const on = S.rt ? s.shown === '1' : s.params && s.params.on === 1
    const mword = s.cell ? machineWord(s0) : null
    const mat = new THREE.MeshStandardMaterial({ color: lit ? '#ffd75e' : mword ? MCOL[mword] : SKCOL[s.kind] || SKCOL.fn, roughness: 0.42, emissive: lit ? 0xb08a10 : sel ? 0x3a2410 : (s.kind === 'src' && on && s.cell) ? 0x403010 : 0x000000 })
    let body, labelY = s.r + 0.9
    if (s.cell && s.kind === 'mast') {
      // a pole from the cell up to the top, where the mast is struck and strikes
      const foot = new THREE.Vector3(s.cell[0] + 0.5, s.cell[1], s.cell[2] + 0.5)
      const top = new THREE.Vector3(...s.pos)
      const pole = new THREE.Mesh(poleGeo, mat)
      pole.position.copy(foot).lerp(top, 0.5).sub(top)
      pole.scale.set(1, foot.distanceTo(top), 1)
      g.add(pole)
      const base = new THREE.Mesh(devGeo, mat)
      base.scale.set(0.7, 0.25, 0.7)
      base.position.set(0, foot.y + 0.125 - top.y, 0)
      base.userData = { sphere: s.id }
      sphereMeshes.push(base)
      g.add(base)
      body = new THREE.Mesh(sphereGeo, mat)
      body.scale.setScalar(0.35)
      labelY = 0.9
    } else if (s.cell) {
      body = new THREE.Mesh(devGeo, mat)
      if (s.kind === 'asm') {
        const fr = new THREE.LineSegments(new THREE.EdgesGeometry(devGeo), new THREE.LineBasicMaterial({ color: 0xfbf6ee }))
        fr.scale.setScalar(0.7)
        g.add(fr)
      }
      labelY = 1.0
    } else if (s.kind === 'port') {
      body = new THREE.Mesh(portGeo, mat)
      body.scale.setScalar(s.r)
    } else {
      body = new THREE.Mesh(sphereGeo, mat)
      body.scale.setScalar(s.r)
      if (s.kind === 'gate') body.scale.set(s.r, s.r, s.r * 0.45)
      if (s.kind === 'asm') {
        const ring = new THREE.LineSegments(new THREE.EdgesGeometry(devGeo), new THREE.LineBasicMaterial({ color: SKCOL.asm }))
        ring.scale.setScalar(s.r * 2.2)
        g.add(ring)
      }
    }
    body.userData = { sphere: s.id }
    sphereMeshes.push(body)
    g.add(body)
    if (!['out', 'port', 'asm'].includes(s.kind) && !mword) {
      const needle = new THREE.Mesh(coneGeo, new THREE.MeshBasicMaterial({ color: TH().accent }))
      const a = new THREE.Vector3(...s.aim)
      needle.position.copy(a.clone().multiplyScalar((s.cell && s.kind !== 'mast' ? 0.42 : s.r) + 0.3))
      needle.quaternion.setFromUnitVectors(up, a)
      needle.scale.set(0.16, 0.5, 0.16)
      g.add(needle)
    }
    let text = mword || s.name
    if (mword) { const q = machineLoad(s.id); if (q) text += '\n' + q }
    if (s.kind === 'ratchet') text += ' ' + (s.notch || 0) + '/' + ((s.params && s.params.n) ?? 5)
    if (s.kind === 'asm') { const sp = S.W.spaces.get(s.body); text += sp && sp.name && sp.name !== s.name ? ' ‹' + sp.name + '›' : ' ‹assembly›' }
    if (s.shown != null) text += '\n' + s.shown
    if (s.error) text += '\n' + s.error
    if (S.layers.labels || s.shown != null || s.error) {
      const lab = T.makeLabel(text, s.error ? TH().err : S.found.has('s' + s.id) ? '#a06a10' : TH().ink, s.cell ? 0.3 : 0.36)
      lab.position.set(0, labelY, 0)
      g.add(lab)
    }
    g.position.set(...s.pos)
    groups.graph.add(g)
  }
  // links: wires on the grid, beams as rays, plain links in the air
  const links = S.rt ? S.rt.links : [...S.W.links.values()].map((l) => ({ ...l, live: l.mode === 'sticky' || (worldHit(l.from) === l.to) }))
  for (const l of S.layers.links ? links : []) {
    const a = sphereView(l.from), b = sphereView(l.to)
    if (!a || !b || !inSpace(a) || !inSpace(b)) continue
    const rec = S.W.links.get(l.id) || l
    const col = new THREE.Color(SKCOL[a.kind] || SKCOL.fn).multiplyScalar(0.8)
    const wire = (rec.mode || l.mode) !== 'ray' && (a.cell || b.cell)
    if (wire) {
      const pts = wirePath(a, b, rec.route || [])
      const curve = new THREE.CurvePath()
      for (let i = 0; i + 1 < pts.length; i++) curve.add(new THREE.LineCurve3(pts[i], pts[i + 1]))
      const tube = new THREE.Mesh(new THREE.TubeGeometry(curve, Math.max(2, pts.length * 4), 0.05, 6, false), new THREE.MeshStandardMaterial({ color: col, roughness: 0.6 }))
      tube.userData = { link: l.id }
      groups.graph.add(tube)
      const end = pts[pts.length - 1], prev = pts[pts.length - 2]
      const d = end.clone().sub(prev).normalize()
      const head = new THREE.Mesh(coneGeo, new THREE.MeshBasicMaterial({ color: col }))
      head.position.copy(end.clone().addScaledVector(d, -0.2))
      head.quaternion.setFromUnitVectors(up, d)
      head.scale.set(0.12, 0.3, 0.12)
      groups.graph.add(head)
      pinLabels(rec, pts[0], end)
      continue
    }
    const A = new THREE.Vector3(...a.pos), B = new THREE.Vector3(...b.pos)
    const d = B.clone().sub(A).normalize()
    const from = A.clone().addScaledVector(d, a.cell ? 0.45 : a.r), to = B.clone().addScaledVector(d, -(b.cell ? 0.45 : b.r))
    const geo = new THREE.BufferGeometry().setFromPoints([from, to])
    const beam = (rec.mode || l.mode) === 'ray'
    const mat = l.live
      ? new THREE.LineBasicMaterial({ color: beam ? new THREE.Color(col).lerp(new THREE.Color(0xffffff), 0.35) : col })
      : new THREE.LineDashedMaterial({ color: col, dashSize: 0.3, gapSize: 0.25, transparent: true, opacity: 0.5 })
    const line = new THREE.Line(geo, mat)
    if (!l.live) line.computeLineDistances()
    line.userData = { link: l.id }
    groups.graph.add(line)
    if (beam && l.live) {
      const glow = new THREE.Mesh(new THREE.CylinderGeometry(0.06, 0.06, from.distanceTo(to), 6, 1, true), new THREE.MeshBasicMaterial({ color: col, transparent: true, opacity: 0.25, depthWrite: false }))
      glow.position.copy(from).lerp(to, 0.5)
      glow.quaternion.setFromUnitVectors(up, d)
      groups.graph.add(glow)
    }
    const head = new THREE.Mesh(coneGeo, new THREE.MeshBasicMaterial({ color: col }))
    head.position.copy(to.clone().addScaledVector(d, -0.2))
    head.quaternion.setFromUnitVectors(up, d)
    head.scale.set(0.12, 0.35, 0.12)
    groups.graph.add(head)
    if (l.port && l.port !== 'out') {
      const pl = T.makeLabel(l.port, TH().accent, 0.26)
      pl.position.copy(from.clone().lerp(to, 0.3))
      groups.graph.add(pl)
    }
    pinLabels(rec, from, to)
  }
  // the ground grid where devices stand
  if (S.gridPlace || [...S.W.spheres.values()].some((x) => inSpace(x) && x.cell)) groups.graph.add(groundGrid())
}

function pinLabels(rec, from, to) {
  if (rec.from_pin) { const t = T.makeLabel('@' + rec.from_pin, '#6a5d8c', 0.22); t.position.copy(from).add(new THREE.Vector3(0, 0.35, 0)); groups.graph.add(t) }
  if (rec.to_pin) { const t = T.makeLabel('@' + rec.to_pin, '#6a5d8c', 0.22); t.position.copy(to).add(new THREE.Vector3(0, 0.35, 0)); groups.graph.add(t) }
}

// a wire runs on the ground: out of its device, along x, then along z, into the next
function wirePath(a, b, route) {
  const lvl = (s) => s.cell ? s.cell[1] + 0.12 : null
  const pts = []
  const A = new THREE.Vector3(...(a.cell && a.kind === 'mast' ? [a.cell[0] + 0.5, a.cell[1] + 0.3, a.cell[2] + 0.5] : a.pos))
  const B = new THREE.Vector3(...(b.cell && b.kind === 'mast' ? [b.cell[0] + 0.5, b.cell[1] + 0.3, b.cell[2] + 0.5] : b.pos))
  pts.push(A)
  if (route.length) {
    for (const c of route) pts.push(new THREE.Vector3(c[0] + 0.5, c[1] + 0.12, c[2] + 0.5))
  } else {
    const y = lvl(a) ?? lvl(b)
    const s0 = new THREE.Vector3(A.x, y, A.z), s2 = new THREE.Vector3(B.x, y, B.z)
    pts.push(s0, new THREE.Vector3(B.x, y, A.z), s2)
  }
  pts.push(B)
  return pts.filter((p, i) => i === 0 || p.distanceToSquared(pts[i - 1]) > 1e-6)
}

let gridMesh = null
function groundGrid() {
  if (!gridMesh) {
    gridMesh = new THREE.GridHelper(64, 64, TH().grid[0], TH().grid[1])
    gridMesh.material.transparent = true
    gridMesh.material.opacity = 0.35
    gridMesh.material.userData.shared = true
    gridMesh.position.set(0, 1.002, 0)
  }
  return gridMesh
}

// a ray in edit mode: nearest accepting sphere (display only; the server makes links)
function worldHit(id) {
  const s = S.W.spheres.get(id)
  if (!s || s.kind === 'out') return null
  let best = null, bt = Infinity
  for (const t of S.W.spheres.values()) {
    if (t.id === id || t.kind === 'src') continue
    const m = [s.pos[0] - t.pos[0], s.pos[1] - t.pos[1], s.pos[2] - t.pos[2]]
    const b = m[0] * s.aim[0] + m[1] * s.aim[1] + m[2] * s.aim[2]
    const q = m[0] * m[0] + m[1] * m[1] + m[2] * m[2] - t.r * t.r
    if (q > 0 && b > 0) continue
    const disc = b * b - q
    if (disc < 0) continue
    let tt = -b - Math.sqrt(disc)
    if (tt < 0) tt = -b + Math.sqrt(disc)
    if (tt >= 0 && tt <= s.reach && tt < bt) { bt = tt; best = t.id }
  }
  return best
}

function buildZones() {
  for (const z of S.W.zones.values()) {
    if (z.kind === 'scope') { buildScope(z); continue }
    const size = [z.max[0] - z.min[0] + 1, z.max[1] - z.min[1] + 1, z.max[2] - z.min[2] + 1]
    const geo = new THREE.BoxGeometry(...size)
    const mesh = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color: ZCOL[z.kind], transparent: true, opacity: S.selZone === z.id ? 0.12 : 0.05, depthWrite: false, side: THREE.DoubleSide }))
    mesh.position.set(z.min[0] + size[0] / 2, z.min[1] + size[1] / 2, z.min[2] + size[2] / 2)
    mesh.userData = { zone: z.id }
    groups.zones.add(mesh)
    const edges = new THREE.LineSegments(new THREE.EdgesGeometry(geo), new THREE.LineBasicMaterial({ color: ZCOL[z.kind] }))
    edges.position.copy(mesh.position)
    groups.zones.add(edges)
    let t = z.name + ' · ' + z.kind
    const rz = S.rt && S.rt.zones.get(z.name)
    if (rz) t += ' · ' + rz.phase + (rz.events ? ' · ' + rz.events + ' events' : '') + (rz.inbox ? ' · inbox ' + rz.inbox : '')
    if (z.captures && z.captures.length) t += '\n' + z.captures.map((c) => c[0] + ':' + c[1]).join(' ')
    const lab = T.makeLabel(t, ZCOL[z.kind], 0.4)
    lab.position.set(z.min[0] + size[0] / 2, z.max[1] + 1.6, z.min[2] + size[2] / 2)
    groups.zones.add(lab)
  }
}

// a scope is painted on the ground: its rectangle, its dialect, its name
function buildScope(z) {
  const d = (z.params && z.params.dialect) || 's'
  const col = DCOL[d] || DCOL.s
  const w = z.max[0] - z.min[0] + 1, dd = z.max[2] - z.min[2] + 1
  const y = 1.004
  const floor = new THREE.Mesh(new THREE.PlaneGeometry(w, dd), new THREE.MeshBasicMaterial({ color: col, transparent: true, opacity: S.selZone === z.id ? 0.3 : 0.16, depthWrite: false, side: THREE.DoubleSide }))
  floor.rotation.x = -Math.PI / 2
  floor.position.set(z.min[0] + w / 2, y, z.min[2] + dd / 2)
  floor.userData = { zone: z.id }
  groups.zones.add(floor)
  const pts = [[z.min[0], z.min[2]], [z.max[0] + 1, z.min[2]], [z.max[0] + 1, z.max[2] + 1], [z.min[0], z.max[2] + 1], [z.min[0], z.min[2]]].map(([x, zz]) => new THREE.Vector3(x, y + 0.002, zz))
  groups.zones.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), new THREE.LineBasicMaterial({ color: col })))
  let t = d + ' · ' + z.name
  const rz = S.rt && S.rt.zones.get(z.name)
  if (rz) t += ' · ' + rz.phase
  const lab = T.makeLabel(t, col, 0.42)
  lab.position.set(z.min[0] + 0.2 + Math.min(4, w / 2), y + 0.5, z.min[2] + 0.3)
  groups.zones.add(lab)
}

// ---------- camera ----------

const cam = { pos: new THREE.Vector3(6, 9, 16), q: new THREE.Quaternion(), speed: 10, keys: new Set(), look: false, orbit: null }
function lookAt(target) {
  const m = new THREE.Matrix4().lookAt(cam.pos, target, new THREE.Vector3(0, 1, 0))
  cam.q.setFromRotationMatrix(m)
}
lookAt(new THREE.Vector3(5, 1, 3))
function frame(points) {
  if (!points.length) return
  const box = new THREE.Box3().setFromPoints(points)
  const c = box.getCenter(new THREE.Vector3())
  const r = Math.max(4, box.getSize(new THREE.Vector3()).length())
  const fwd = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
  cam.pos.copy(c).addScaledVector(fwd, -r * 0.75).add(new THREE.Vector3(0, r * 0.2, 0))
  lookAt(c)
}
function frameSpace() {
  const pts = []
  for (const b of S.W.bricks.values()) if (b.space === S.space) pts.push(new THREE.Vector3(b.cell[0] + 0.5, b.cell[1] + 0.5, b.cell[2] + 0.5))
  for (const p of platesIn(S.space)) { pts.push(new THREE.Vector3(p.at[0], p.at[1] + 1, p.at[2]), new THREE.Vector3(p.at[0] + p.w, p.at[1] + 1, p.at[2] + p.d)) }
  for (const s of S.W.spheres.values()) if (inSpace(s)) pts.push(new THREE.Vector3(...s.pos))
  for (const d of S.W.datums.values()) if ((d.space || 0) === S.space) pts.push(new THREE.Vector3(d.cell[0] + 0.5, d.cell[1] + 0.5, d.cell[2] + 0.5))
  frame(pts)
}

// ---------- picking ----------

const ray = new THREE.Raycaster()
const ndc = new THREE.Vector2()
let mouse = { x: 0, y: 0 }
function pick(cx, cy) {
  const r = canvas.getBoundingClientRect()
  ndc.set(((cx - r.left) / r.width) * 2 - 1, -((cy - r.top) / r.height) * 2 + 1)
  ray.setFromCamera(ndc, camera)
  const objs = [...sphereMeshes, ...trackMeshes]
  if (brickMesh) objs.push(brickMesh)
  objs.push(...plateMeshes)
  const hits = ray.intersectObjects(objs, false)
  const dh = S.W.datums.size ? nearestDatum(ray.ray.origin.toArray(), ray.ray.direction.toArray()) : null
  for (const x of hits) {
    if (dh && dh.t < x.distance) return datumPick(dh)
    if (x.object.userData.sphere) return { sphere: x.object.userData.sphere, point: x.point }
    if (x.object.userData.track) {
      const t = S.W.tracks.get(x.object.userData.track), c = t && t.cells[x.instanceId]
      if (c) return { track: t.id, point: x.point, cell: [c[0], c[1] + 1, c[2]], on: c }
    }
    if (x.object === brickMesh) {
      const b = S.W.bricks.get(brickIds[x.instanceId])
      if (!b) continue
      const n = x.face.normal.clone().round()
      return { brick: b.id, point: x.point, cell: [b.cell[0] + n.x, b.cell[1] + n.y, b.cell[2] + n.z] }
    }
    if (x.object.userData.plate) {
      const p = S.W.plates.get(x.object.userData.plate)
      return { plate: p.id, point: x.point, cell: [Math.floor(x.point.x), p.at[1] + 1, Math.floor(x.point.z)] }
    }
  }
  if (dh) return datumPick(dh)
  // the zone walls, last
  if (S.space === 0) {
    const zh = ray.intersectObjects(groups.zones.children.filter((o) => o.userData.zone), false)
    if (zh.length) return { zone: zh[0].object.userData.zone, point: zh[0].point }
  }
  // the ground, where devices stand without a plate
  const g = ray.ray.intersectPlane(new THREE.Plane(new THREE.Vector3(0, 1, 0), -1), new THREE.Vector3())
  if (g && g.distanceTo(camera.position) < 200) return { ground: true, point: g, cell: [Math.floor(g.x), 1, Math.floor(g.z)] }
  return { point: ray.ray.at(12, new THREE.Vector3()) }
}

const ghost = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(1, 1, 1)), new THREE.LineBasicMaterial({ color: 0x7a5532 }))
ghost.visible = false
groups.over.add(ghost)
const hoverRing = new THREE.LineSegments(new THREE.EdgesGeometry(new THREE.BoxGeometry(1.02, 1.02, 1.02)), new THREE.LineBasicMaterial({ color: 0x3b2d20 }))
hoverRing.visible = false
groups.over.add(hoverRing)

function placeable(cell) {
  return cell && !brickAt(S.space, cell) && !datumAt(S.space, cell) && plateUnder(S.space, cell)
}

// ---------- the word editor: typing upward ----------

const ed = { open: false, space: 0, cell: null, items: [], pick: 0, brick: null }
const edBox = $('#editor'), edIn = $('#ed-input'), edList = $('#ed-list')

function openEditor(space, cell, brick = null) {
  ed.open = true
  ed.space = space
  ed.cell = cell
  ed.brick = brick
  edBox.hidden = false
  edIn.value = brick ? brickText(S.W.bricks.get(brick)) : (S.armed || '')
  S.armed = null
  placeEditor()
  // after the click that opened it has finished moving focus
  setTimeout(() => { edIn.focus(); edIn.select() }, 0)
  complete()
}
function closeEditor() {
  ghost.visible = false
  ed.open = false
  edBox.hidden = true
  edList.innerHTML = ''
  canvas.focus()
}
function cellScreen(c) {
  const v = new THREE.Vector3(c[0] + 0.5, c[1] + 0.5, c[2] + 0.5).project(camera)
  const r = canvas.getBoundingClientRect()
  return [(v.x + 1) / 2 * r.width + r.left, (1 - v.y) / 2 * r.height + r.top, v.z < 1]
}
function placeEditor() {
  if (!ed.open) return
  const [x, y, vis] = cellScreen(ed.cell)
  edBox.style.left = Math.max(4, Math.min(innerWidth - 350, x - 90)) + 'px'
  edBox.style.top = Math.max(60, Math.min(innerHeight - 260, y - 14)) + 'px'
  edBox.style.opacity = vis ? 1 : 0.5
  ghost.visible = true
  ghost.position.set(ed.cell[0] + 0.5, ed.cell[1] + 0.5, ed.cell[2] + 0.5)
}
let compTimer = 0
function complete() {
  clearTimeout(compTimer)
  compTimer = setTimeout(async () => {
    const prefix = edIn.value.trim()
    const r = await api('complete', { prefix, brick: ed.brick || 0 })
    ed.items = (r.items || []).slice(0, 12)
    ed.pick = 0
    edList.innerHTML = ''
    if (!prefix) return
    ed.items.forEach((it, i) => {
      edList.append(h('div', { class: 'item' + (i === 0 ? ' on' : ''), onmousedown: (e) => { e.preventDefault(); edIn.value = it.w; commitEditor() } }, h('code', { class: 'k-' + it.kind, text: it.w }), h('span', { text: it.doc })))
    })
  }, 60)
}
async function commitEditor(next = 'up') {
  const word = edIn.value.trim()
  if (!word) { closeEditor(); return }
  const cell = ed.cell
  const r = await edit({ op: 'place', space: ed.space, cell, word })
  if (!r) { edIn.select(); return }
  // multi-word input lays out upward: one brick per word
  const n = r.id && S.W.bricks.get(r.id) ? 1 : 1
  if (next === 'up') {
    let c = [cell[0], cell[1] + n, cell[2]]
    while (brickAt(ed.space, c)) c = [c[0], c[1] + 1, c[2]]
    ed.cell = c
    ed.brick = null
    edIn.value = ''
    placeEditor()
  } else if (next === 'col') {
    const p = plateUnder(ed.space, cell)
    const step = { '+x': [1, 0], '-x': [-1, 0], '+z': [0, 1], '-z': [0, -1] }[p ? p.arrow : '+x']
    ed.cell = [cell[0] + step[0], p ? p.at[1] + 1 : cell[1], cell[2] + step[1]]
    ed.brick = null
    edIn.value = ''
    placeEditor()
  }
  edList.innerHTML = ''
}
edIn.addEventListener('input', complete)
edIn.addEventListener('keydown', (e) => {
  e.stopPropagation()
  if (e.key === 'F5' || e.key === 'F6' || e.key === 'F9' || e.key === 'F10') return
  const items = [...edList.children]
  if (e.key === 'Escape') { e.preventDefault(); closeEditor() }
  else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) { e.preventDefault(); commitEditor('col') }
  else if (e.key === 'Enter') { e.preventDefault(); commitEditor('up') }
  else if (e.key === 'Tab') {
    e.preventDefault()
    if (ed.items[ed.pick]) { edIn.value = ed.items[ed.pick].w; complete() }
  } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    if (!items.length) return
    e.preventDefault()
    ed.pick = (ed.pick + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length
    items.forEach((x, i) => x.classList.toggle('on', i === ed.pick))
  }
})

// ---------- the line bar ----------

const lb = { brick: null }
const lbIn = $('#lb-input'), lbHl = $('#lb-hl')
function openLine(b) {
  lb.brick = b.id
  $('#linebar').hidden = false
  lbIn.value = columnOf(b).map(brickText).join(' ')
  colorLine()
  lbIn.focus()
}
let lexTimer = 0
function colorLine() {
  clearTimeout(lexTimer)
  lexTimer = setTimeout(async () => {
    const t = lbIn.value
    const r = await fetch('api', { method: 'POST', body: JSON.stringify({ cmd: 'lex', text: t }) }).then((x) => x.json())
    let html = '', at = 0
    const esc = (s) => s.replace(/[&<>]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;' }[c]))
    const cs = [...t]
    for (const k of r.tokens || []) {
      html += esc(cs.slice(at, k.from).join('')) + '<span class="k-' + k.kind + '">' + esc(cs.slice(k.from, k.to).join('')) + '</span>'
      at = k.to
    }
    lbHl.innerHTML = html + esc(cs.slice(at).join(''))
    $('#lb-msg').textContent = r.error && r.error.msg ? r.error.msg : ''
  }, 40)
}
lbIn.addEventListener('input', colorLine)
lbIn.addEventListener('keydown', async (e) => {
  e.stopPropagation()
  if (e.key === 'Escape') { $('#linebar').hidden = true; canvas.focus() }
  if (e.key === 'Enter') {
    const b = S.W.bricks.get(lb.brick)
    if (!b) return
    const base = columnOf(b)[0]
    const r = await edit({ op: 'column', space: b.space, cell: base.cell, line: lbIn.value })
    if (r) { $('#linebar').hidden = true; canvas.focus() }
  }
})

// ---------- selection and inspector ----------

function selectBrick(id, add = false) {
  if (!add) S.sel.clear()
  if (add && S.sel.has(id)) S.sel.delete(id)
  else S.sel.add(id)
  S.selSphere = null
  S.selDatum = null
  S.selZone = null
  S.selPlate = null
  dirty()
  refreshInspector()
}

const ins = $('#inspector'), insBody = $('#ins-body')
function field(label, input) { return h('div', { class: 'field' }, h('span', { class: 'lab', text: label }), input) }

async function refreshInspector() {
  if (ins.hidden) return
  insBody.innerHTML = ''
  if (S.selSphere) return sphereInspector(S.W.spheres.get(S.selSphere))
  if (S.selDatum) return datumInspector(datumView(S.selDatum))
  if (S.selZone) return zoneInspector(S.W.zones.get(S.selZone))
  if (S.selPlate) return plateInspector(S.W.plates.get(S.selPlate))
  if (S.sel.size === 1) return brickInspector(S.W.bricks.get([...S.sel][0]))
  if (S.W.goal) return goalInspector()
  $('#ins-title').textContent = 'nothing selected'
  insBody.append(h('div', { class: 'note', text: 'Click a brick, a sphere, a plate edge or a zone wall. E opens this panel.' }))
}

async function brickInspector(b) {
  if (!b) return
  $('#ins-title').textContent = 'brick ' + brickText(b)
  const r = await api('resolve', { brick: b.id })
  insBody.innerHTML = ''
  insBody.append(h('div', { class: 'doc', text: [r.kind, r.sig, r.doc, r.file].filter(Boolean).join('\n') }))
  if (r.kind === 'def' && r.column) insBody.append(h('button', { class: 'btn', text: 'Show the definition', onclick: () => { S.sel = new Set(r.column); dirty(); flyTo(S.W.bricks.get(r.column[0])) } }))
  const capSel = h('select', { onchange: () => edit({ op: 'set', id: b.id, cap: capSel.value }) }, ...['', '~', '`', '#', ';', ':', '$', '.', '..', '@'].map((c) => h('option', { value: c, text: c || '(none)' })))
  capSel.value = b.cap || ''
  const condSel = h('select', { onchange: () => edit({ op: 'set', id: b.id, cond: condSel.value }) }, ...[['', 'always'], ['?', '? when ow'], ['!', '! unless ow']].map(([v, t]) => h('option', { value: v, text: t })))
  condSel.value = b.cond || ''
  const note = h('input', { value: b.note || '', onchange: () => edit({ op: 'set', id: b.id, note: note.value }) })
  insBody.append(field('kind', h('span', { text: b.kind })), field('cap', capSel), field('collar', condSel), field('note', note))
  insBody.append(h('div', { class: 'row' },
    h('button', { class: 'btn', text: S.breaks.has(b.id) ? 'Clear breakpoint' : 'Breakpoint (F8)', onclick: () => toggleBreak(b.id) }),
    b.kind === 'box' && !b.word ? h('button', { class: 'btn', text: 'Open (Space)', onclick: () => enterSpace(b.inner) }) : null,
    h('button', { class: 'btn', text: 'Column text', onclick: () => openLine(b) })))
}

function sphereInspector(s) {
  if (!s) return
  $('#ins-title').textContent = s.kind + ' ' + s.name
  const put = (patch) => edit({ op: 'put', t: 'sphere', ...s, ...patch })
  const name = h('input', { value: s.name, onchange: () => put({ name: name.value.trim() }) })
  const kind = h('select', { onchange: () => put({ kind: kind.value }) }, ...['src', 'fn', 'ratchet', 'out', 'gate'].map((k) => h('option', { value: k, text: k })))
  kind.value = s.kind
  const strike = h('select', { onchange: () => put({ strike: strike.value }) }, h('option', { value: 'sticky', text: 'sticky' }), h('option', { value: 'ray', text: 'ray' }))
  strike.value = s.strike
  const reach = h('input', { value: s.reach, onchange: () => put({ reach: +reach.value }) })
  const r = h('input', { value: s.r, onchange: () => put({ r: +r.value }) })
  insBody.append(field('name', name), field('kind', kind), field('strike', strike), field('reach', reach), field('radius', r))
  const params = { ...(s.params || {}) }
  const keys = s.kind === 'ratchet' ? ['n'] : s.kind === 'src' ? ['every', 'run'] : []
  for (const k of keys) {
    const inp = h('input', { value: params[k] ?? '', onchange: () => { params[k] = +inp.value; put({ params }) } })
    insBody.append(field(k, inp))
  }
  insBody.append(h('div', { class: 'note', text: 'aim ' + s.aim.map((x) => x.toFixed(2)).join(' ') + ' · Alt+drag onto a sphere to aim at it' }))
  // body
  insBody.append(h('b', { text: 'Body' }))
  const ta = h('textarea', { class: 'code', spellcheck: 'false' })
  ta.value = spaceText(s.body)
  insBody.append(ta, h('div', { class: 'row' },
    h('button', { class: 'btn', text: 'Save body', onclick: () => edit({ op: 'settext', space: s.body, text: ta.value }) }),
    h('button', { class: 'btn', text: 'Open as bricks (Space)', onclick: () => enterSpace(s.body) }),
    S.run ? h('button', { class: 'btn', text: 'Fire', onclick: () => api('fire', { sphere: s.id }).then(() => startLoop()) }) : null))
  // relations
  insBody.append(h('b', { text: 'Relations' }))
  const links = [...S.W.links.values()].filter((l) => l.from === s.id || l.to === s.id)
  if (!links.length) insBody.append(h('div', { class: 'note', text: 'No links. Aim at a sphere and the strike makes one.' }))
  for (const l of links) {
    const other = S.W.spheres.get(l.from === s.id ? l.to : l.from)
    const live = l.mode === 'sticky' || worldHit(l.from) === l.to
    const mode = h('select', { onchange: () => edit({ op: 'put', t: 'link', ...l, mode: mode.value }) }, h('option', { value: 'sticky', text: 'sticky' }), h('option', { value: 'ray', text: 'ray' }))
    mode.value = l.mode
    const port = S.W.spheres.get(l.from).kind === 'ratchet' ? h('select', { onchange: () => edit({ op: 'put', t: 'link', ...l, port: port.value }) }, ...['loop', 'exit'].map((p) => h('option', { value: p, text: p }))) : null
    if (port) port.value = l.port
    const pinSel = (side) => {
      const sp = S.W.spheres.get(side === 'from' ? l.from : l.to)
      if (!sp || sp.kind !== 'asm') return null
      const ports = [...S.W.spheres.values()].filter((x) => x.space === sp.body && x.kind === 'port').map((x) => x.name.split('/').pop())
      const sel = h('select', { title: 'port of ' + sp.name, onchange: () => edit({ op: 'put', t: 'link', ...l, [side + '_pin']: sel.value }) }, h('option', { value: '', text: '@first' }), ...ports.map((x) => h('option', { value: x, text: '@' + x })))
      sel.value = l[side + '_pin'] || ''
      return sel
    }
    insBody.append(h('div', { class: 'lrow' + (live ? '' : ' dormant') },
      h('span', { class: 'name', text: (l.from === s.id ? '→ ' : '← ') + (other ? other.name : '?') + (l.mode === 'ray' ? ' (beam)' : (s.cell || (other && other.cell)) ? ' (wire)' : '') }), mode, port, pinSel('from'), pinSel('to'),
      h('button', { class: 'x', title: 'Remove', text: '×', onclick: () => edit({ op: 'remove', ids: [l.id] }) })))
  }
  if (s.kind === 'src' && s.params && 'on' in s.params) {
    insBody.append(h('div', { class: 'row' }, h('button', { class: 'btn', text: s.params.on ? 'Switch off' : 'Switch on', onclick: () => toggleSwitch(s) })))
  }
  const layer = s.kind === 'gate' ? null : s.cell
    ? (s.kind === 'mast' ? null : h('button', { class: 'btn', text: 'Lift to the sphere layer', onclick: () => edit({ op: 'lift', ids: [s.id] }) }))
    : h('button', { class: 'btn', text: 'Set down on the grid', onclick: () => edit({ op: 'setdown', ids: [s.id] }) })
  insBody.append(h('div', { class: 'row' }, layer,
    s.kind === 'asm' ? h('button', { class: 'btn', text: 'Open inside (Space)', onclick: () => enterSpace(s.body) }) : null,
    h('button', { class: 'btn', text: 'Remove', onclick: () => { edit({ op: 'remove', ids: [s.id] }); S.selSphere = null } })))
  if (s.kind === 'asm') {
    const ports = [...S.W.spheres.values()].filter((x) => x.space === s.body && x.kind === 'port').map((x) => x.name.split('/').pop())
    insBody.append(h('div', { class: 'note', text: 'ports: ' + (ports.join(', ') || 'none') + ' · links to and from a placement name a port with @' }))
  }
  if (s.cell) insBody.append(h('div', { class: 'note', text: 'on the grid at ' + s.cell.join(' ') + (s.kind === 'mast' ? ' · its top is ' + ((s.params && s.params.h) ?? 4) + ' up' : '') }))
  assembleRow()
}

function toggleSwitch(s) {
  const params = { ...(s.params || {}), on: s.params && s.params.on ? 0 : 1 }
  return edit({ op: 'put', t: 'sphere', ...s, params })
}

function assembleRow() {
  const ids = new Set(S.selSpheres)
  if (S.selSphere) ids.add(S.selSphere)
  if (ids.size < 1) return
  const nm = h('input', { placeholder: 'assembly name' })
  insBody.append(h('b', { text: 'Assemble ' + ids.size + (ids.size === 1 ? ' sphere' : ' spheres') }),
    h('div', { class: 'note', text: 'Shift+click adds spheres. Links that cross the edge become ports.' }),
    h('div', { class: 'row' }, nm, h('button', { class: 'btn', text: 'Assemble', onclick: async () => {
      const n = nm.value.trim()
      if (!n) { status('an assembly needs a name', true); return }
      const r = await edit({ op: 'assemble', ids: [...ids], name: n })
      if (r && r.id) { S.selSpheres.clear(); S.selSphere = r.id; refreshInspector() }
    } })))
}

function zoneInspector(z) {
  if (!z) return
  $('#ins-title').textContent = 'zone ' + z.name
  const put = (patch) => edit({ op: 'put', t: 'zone', ...z, ...patch })
  const name = h('input', { value: z.name, onchange: () => put({ name: name.value.trim() }) })
  const kind = h('select', { onchange: () => put({ kind: kind.value }) }, ...['async', 'thread', 'net', 'rpc', 'scope'].map((k) => h('option', { value: k, text: k })))
  kind.value = z.kind
  const min = h('input', { value: z.min.join(' '), onchange: () => put({ min: min.value.trim().split(/\s+/).map(Number) }) })
  const max = h('input', { value: z.max.join(' '), onchange: () => put({ max: max.value.trim().split(/\s+/).map(Number) }) })
  insBody.append(field('name', name), field('kind', kind), field('min', min), field('max', max))
  const rz = S.rt && S.rt.zones.get(z.name)
  if (rz) {
    const held = (rz.held || []).map(([n, m]) => n + ' as ' + m).join(', ')
    insBody.append(h('div', { class: 'doc', text: rz.phase + ' · ' + rz.events + ' events · ' + rz.queued + ' queued · inbox ' + rz.inbox + (held ? '\nholds ' + held : '') }))
  }
  const params = { ...(z.params || {}) }
  for (const k of ['dialect', 'merge', 'budget', 'host', 'at', 'export', 'peer', 'image']) {
    if (['at', 'export', 'peer', 'image'].includes(k) && z.kind !== 'rpc') continue
    if (k === 'dialect' && z.kind !== 'scope') continue
    const inp = h('input', { value: params[k] ?? '', placeholder: { merge: 'arrival or round', budget: '100000', host: 'host:port for net', at: 'core or host:port', export: 'true offers the gates to coupled worlds', peer: 'host:port of a world to couple to', image: 'none with a peer' }[k], onchange: () => { if (inp.value) params[k] = inp.value; else delete params[k]; put({ params }) } })
    insBody.append(field(k, inp))
  }
  insBody.append(h('b', { text: 'Captures' }))
  const caps = (z.captures || []).map((c) => [...c])
  caps.forEach((c, i) => {
    const mode = h('select', { onchange: () => { caps[i][1] = mode.value; put({ captures: caps }) } }, ...['copy', 'move', 'ref', 'mut'].map((m) => h('option', { value: m, text: m })))
    mode.value = c[1]
    insBody.append(h('div', { class: 'lrow' }, h('code', { class: 'name', text: c[0] }), mode, h('button', { class: 'x', text: '×', onclick: () => { caps.splice(i, 1); put({ captures: caps }) } })))
  })
  const nn = h('input', { placeholder: 'name to capture' })
  insBody.append(h('div', { class: 'row' }, nn, h('button', { class: 'btn', text: 'Add', onclick: () => { if (nn.value.trim()) { caps.push([nn.value.trim(), 'copy']); put({ captures: caps }) } } })))
  insBody.append(h('div', { class: 'note', text: 'copy: the zone gets its own · move: the parent loses it · ref: both read only until join · mut: the zone owns it and gives it back at join. Gates are gate spheres on the walls.' }))
  insBody.append(h('div', { class: 'row' }, h('button', { class: 'btn', text: 'Remove zone', onclick: () => { edit({ op: 'remove', ids: [z.id] }); S.selZone = null } })))
}

function plateInspector(p) {
  if (!p) return
  $('#ins-title').textContent = p.kind + ' plate'
  const put = (patch) => edit({ op: 'put', t: 'plate', ...p, ...patch })
  const kind = h('select', { onchange: () => put({ kind: kind.value }) }, ...['main', 'script', 'shelf', 'inside'].map((k) => h('option', { value: k, text: k })))
  kind.value = p.kind
  const name = h('input', { value: p.name, onchange: () => put({ name: name.value.trim() }) })
  const w = h('input', { value: p.w, onchange: () => put({ w: +w.value }) })
  const d = h('input', { value: p.d, onchange: () => put({ d: +d.value }) })
  const arrow = h('select', { onchange: () => put({ arrow: arrow.value }) }, ...['+x', '-x', '+z', '-z'].map((a) => h('option', { value: a, text: a })))
  arrow.value = p.arrow
  insBody.append(field('kind', kind), field('name', name), field('width', w), field('depth', d), field('arrow', arrow))
  insBody.append(h('div', { class: 'note', text: 'A script plate is a script called name. Shelves run first, lowest first: height is generation.' }))
  insBody.append(h('div', { class: 'doc', text: plateColumns(p).map((c) => c.map(brickText).join(' ')).join('\n') || '(empty)' }))
  insBody.append(h('div', { class: 'row' }, h('button', { class: 'btn', text: 'Remove plate and its bricks', onclick: () => { edit({ op: 'remove', ids: [p.id] }); S.selPlate = null } })))
}

function goalInspector() {
  const g = S.W.goal
  $('#ins-title').textContent = g.title || 'goal'
  insBody.append(h('div', { class: 'doc', text: g.text || '' }))
  if (g.output) insBody.append(h('div', { class: 'note', text: 'prints: ' + JSON.stringify(g.output) }))
  for (const [k, v] of Object.entries(g.shown || {})) insBody.append(h('div', { class: 'note', text: k + ' shows: ' + v }))
  if (g.par) insBody.append(h('div', { class: 'note', text: 'par: ' + Object.entries(g.par).map(([k, v]) => v + ' ' + k).join(', ') }))
  insBody.append(h('button', { class: 'btn', text: 'Check', onclick: checkGoal }))
}

function openInspector() { ins.hidden = false; refreshInspector() }
$('#ins-close').onclick = () => { ins.hidden = true }

function flyTo(b) {
  if (!b) return
  if (b.space !== S.space) enterSpace(b.space)
  const c = new THREE.Vector3(b.cell[0] + 0.5, b.cell[1] + 0.5, b.cell[2] + 0.5)
  const fwd = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
  cam.pos.copy(c).addScaledVector(fwd, -7)
  lookAt(c)
}

// ---------- spaces: boxes and sphere bodies are rooms ----------

const spaceStack = []
function enterSpace(sp) {
  if (sp === S.space) return
  if (!S.W.spaces.has(sp) && sp !== 0) return
  spaceStack.push({ space: S.space, pos: cam.pos.clone(), q: cam.q.clone() })
  S.space = sp
  S.sel.clear()
  closeEditor()
  dirty()
  build()
  frameSpace()
}
function leaveSpace() {
  const top = spaceStack.pop()
  if (!top) return false
  S.space = top.space
  cam.pos.copy(top.pos)
  cam.q.copy(top.q)
  S.sel.clear()
  closeEditor()
  dirty()
  return true
}
function spaceName(sp) {
  const s = S.W.spaces.get(sp)
  if (!s) return 'world'
  if (s.owner === 'box') { const b = S.W.bricks.get(s.of); return 'box ' + (b ? b.id : '') }
  if (s.owner === 'sphere') { const x = S.W.spheres.get(s.of); return 'body of ' + (x ? x.name : '') }
  if (s.owner === 'assembly') return 'assembly ' + s.name
  return 'space ' + sp
}
function renderCrumbs() {
  const c = $('#crumbs')
  c.innerHTML = ''
  if (S.space === 0) return
  const parts = [...spaceStack.map((x) => x.space), S.space]
  parts.forEach((sp, i) => {
    if (i) c.append(' › ')
    const a = h(i < parts.length - 1 ? 'a' : 'span', { text: spaceName(sp) })
    if (i < parts.length - 1) a.onclick = () => { while (spaceStack.length > i) leaveSpace() }
    c.append(a)
  })
}

// ---------- input ----------

let drag = null
canvas.addEventListener('contextmenu', (e) => e.preventDefault())
canvas.addEventListener('pointerdown', (e) => {
  canvas.focus()
  mouse = { x: e.clientX, y: e.clientY }
  if (walk.on) { walkClick(e); return }
  if (grab.on) { e.preventDefault(); if (e.button === 0) commitGrab(); else cancelGrab(); return }
  if (e.button === 0 && gimbalDown(e)) return
  if (e.button === 0 && (e.ctrlKey || e.metaKey) && !S.tool) {
    drag = { kind: 'marquee', x: e.clientX, y: e.clientY, add: e.shiftKey }
    canvas.setPointerCapture(e.pointerId)
    return
  }
  if (e.button === 2) {
    drag = { kind: 'look', x: e.clientX, y: e.clientY, t: performance.now(), moved: 0 }
    cam.look = true
    canvas.setPointerCapture(e.pointerId)
    return
  }
  if (e.button === 1) {
    drag = { kind: 'pan', x: e.clientX, y: e.clientY }
    canvas.setPointerCapture(e.pointerId)
    e.preventDefault()
    return
  }
  if (e.button !== 0) return
  const p = pick(e.clientX, e.clientY)
  if (S.tool === 'datum') {
    const c = p.cell
    if (!c || p.sphere || p.brick && !placeable(c) && !p.datum && !p.ground) { status('click the ground, a plate or a face to set a cube against', true); return }
    edit({ op: 'datum', cell: c, space: S.space }).then((r) => { if (r && r.id) { S.selDatum = r.id; S.selSphere = null; S.sel.clear(); dirty(); refreshInspector() } })
    if (!e.shiftKey) setTool(null)
    return
  }
  if (p.datum && !S.tool) {
    const was = S.selDatum
    S.selDatum = p.datum
    S.sel.clear(); S.selSphere = null; S.selSpheres.clear(); S.selZone = null; S.selPlate = null
    // a click on a selected cube toggles the tile; Shift only selects
    if (!e.shiftKey && (was === p.datum || e.altKey || walk.on)) toggleTile(p)
    else status('datum ' + S.W.datums.get(p.datum).name + ' selected: click a tile to toggle its bit · T takes the gimbal to scale and turn it')
    dirty()
    refreshInspector()
    return
  }
  if (S.tool === 'wire') {
    if (!p.sphere) { status('click a sphere or device to wire from, then one to wire to', true); return }
    if (!S.wireFrom) { S.wireFrom = p.sphere; status('wire from ' + S.W.spheres.get(p.sphere).name + ': click where it goes'); return }
    const from = S.wireFrom
    S.wireFrom = null
    if (from !== p.sphere) edit({ op: 'link', from, to: p.sphere })
    if (!e.shiftKey) setTool(null)
    return
  }
  if (S.tool && S.tool.startsWith('scope:')) {
    if (!p.cell) { status('click a cell on the ground or a plate', true); return }
    if (!S.scopeFrom) { S.scopeFrom = p.cell; status('scope from ' + p.cell[0] + ' ' + p.cell[2] + ': click the opposite corner'); return }
    const a = S.scopeFrom, b = p.cell, d = S.tool.slice(6)
    S.scopeFrom = null
    let i = 1
    while ([...S.W.zones.values()].some((z) => z.name === d + i)) i++
    edit({ op: 'zone', name: d + i, kind: 'scope', min: [Math.min(a[0], b[0]), -4, Math.min(a[2], b[2])], max: [Math.max(a[0], b[0]), 40, Math.max(a[2], b[2])], params: { dialect: d } }).then((r) => { if (r && r.id) { S.selZone = r.id; openInspector() } })
    if (!e.shiftKey) setTool(null)
    return
  }
  if (S.tool === 'track') {
    if (!p.cell || p.sphere) { status('click the cell a track starts at, on the ground or a plate', true); return }
    trackClick(p.cell, e.shiftKey)
    return
  }
  if (S.tool && S.tool.startsWith('machine:')) {
    if (!p.cell || p.sphere || p.track) { status('a machine stands on a free cell, past a track\'s end', true); return }
    placeMachine(S.tool.slice(8), p.cell)
    if (!e.shiftKey) setTool(null)
    return
  }
  if (S.tool === 'switch') {
    if (!p.cell || p.sphere) { status('click a cell on the ground or a plate', true); return }
    edit({ op: 'sphere', kind: 'src', cell: p.cell, aim: [1, 0, 0], reach: 0.01, params: { on: 0, run: 0 }, code: '', space: S.space })
    if (!e.shiftKey) setTool(null)
    return
  }
  if (S.tool) {
    // place a sphere, a device or an assembly of the armed kind where the cursor points
    const fwd = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
    fwd.y = 0
    fwd.normalize()
    const aim = Math.abs(fwd.x) > Math.abs(fwd.z) ? [Math.sign(fwd.x), 0, 0] : [0, 0, Math.sign(fwd.z) || 1]
    const grid = (S.gridPlace || S.tool === 'mast') && p.cell && !p.sphere
    const pos = p.cell ? [p.cell[0] + 0.5, p.cell[1] + 1.5, p.cell[2] + 0.5] : [p.point.x, p.point.y + (p.plate ? 1.5 : 0), p.point.z]
    const where = grid ? { cell: p.cell, aim } : { pos: pos.map((x) => Math.round(x * 2) / 2), aim: [fwd.x, 0, fwd.z] }
    const e2 = S.tool.startsWith('asm:') ? { op: 'instance', assembly: S.tool.slice(4), space: S.space, ...where } : { op: 'sphere', kind: S.tool, space: S.space, ...where }
    if (S.tool === 'mast' && !grid) { status('a mast stands on a cell: point at a plate or the ground', true); return }
    edit(e2).then((r) => { if (r && r.id) { S.selSphere = r.id; openInspector() } })
    if (!e.shiftKey) setTool(null)
    return
  }
  if (p.sphere) {
    const s = S.W.spheres.get(p.sphere)
    if (e.shiftKey) {
      if (S.selSphere) S.selSpheres.add(S.selSphere)
      S.selSpheres.has(p.sphere) ? S.selSpheres.delete(p.sphere) : S.selSpheres.add(p.sphere)
    } else S.selSpheres.clear()
    S.selSphere = p.sphere
    S.sel.clear(); S.selZone = null; S.selPlate = null; S.selDatum = null
    dirty()
    refreshInspector()
    drag = { kind: e.altKey ? 'aim' : 'sphere', id: p.sphere, x: e.clientX, y: e.clientY, start: new THREE.Vector3(...s.pos), depth: camera.position.distanceTo(new THREE.Vector3(...s.pos)), moved: 0 }
    canvas.setPointerCapture(e.pointerId)
    return
  }
  if (p.brick) {
    if (e.detail >= 2) { openLine(S.W.bricks.get(p.brick)); return }
    if (e.altKey || (S.armed && !e.shiftKey)) {
      if (placeable(p.cell)) { e.preventDefault(); openEditor(S.space, p.cell) }
      return
    }
    selectBrick(p.brick, e.shiftKey)
    return
  }
  if (p.plate) {
    if (e.shiftKey) { S.selPlate = p.plate; S.sel.clear(); S.selSphere = S.selDatum = null; dirty(); openInspector(); return }
    if (placeable(p.cell)) { e.preventDefault(); openEditor(S.space, p.cell) }
    return
  }
  if (p.zone) { S.selZone = p.zone; S.sel.clear(); S.selSphere = S.selDatum = null; dirty(); openInspector(); return }
  S.sel.clear(); S.selSphere = null; S.selSpheres.clear(); S.selZone = null; S.selPlate = null; S.selDatum = null
  closeEditor()
  dirty()
  refreshInspector()
  // a drag from empty space selects the bricks inside its rectangle
  drag = { kind: 'marquee', x: e.clientX, y: e.clientY, add: false }
  canvas.setPointerCapture(e.pointerId)
})

canvas.addEventListener('pointermove', (e) => {
  const dx = e.clientX - mouse.x, dy = e.clientY - mouse.y
  mouse = { x: e.clientX, y: e.clientY }
  if (grab.on && !drag) { updateGrab(e.clientX, e.clientY); return }
  if (drag && drag.kind === 'gimbal') { gimbalMove(e); return }
  if (!drag) { hoverAt(e.clientX, e.clientY); return }
  if (drag.kind === 'marquee') { showMarquee(drag, e.clientX, e.clientY); return }
  if (drag.kind === 'look' && cam.orbit) {
    drag.moved += Math.abs(dx) + Math.abs(dy)
    orbitBy(-dx * 0.006, -dy * 0.006)
    return
  }
  if (drag.kind === 'look') {
    drag.moved += Math.abs(dx) + Math.abs(dy)
    const yaw = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 1, 0), -dx * 0.0025)
    const pitch = new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1, 0, 0), -dy * 0.0025)
    cam.q.multiply(yaw).multiply(pitch)
  } else if (drag.kind === 'pan') {
    const r = new THREE.Vector3(1, 0, 0).applyQuaternion(cam.q), u = new THREE.Vector3(0, 1, 0).applyQuaternion(cam.q)
    cam.pos.addScaledVector(r, -dx * 0.02).addScaledVector(u, dy * 0.02)
  } else if (drag.kind === 'sphere') {
    drag.moved += Math.abs(dx) + Math.abs(dy)
    const s = S.W.spheres.get(drag.id)
    if (!s || s.kind === 'gate' && false) return
    const r = canvas.getBoundingClientRect()
    ndc.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1)
    ray.setFromCamera(ndc, camera)
    const p = ray.ray.at(drag.depth, new THREE.Vector3())
    drag.to = p
    const g = groups.graph.children.find((c) => c.children[0] && c.children[0].userData.sphere === drag.id)
    if (g) g.position.copy(p)
  } else if (drag.kind === 'aim') {
    drag.moved += Math.abs(dx) + Math.abs(dy)
    const p = pick(e.clientX, e.clientY)
    drag.target = p.sphere && p.sphere !== drag.id ? p.sphere : null
    status(drag.target ? 'aim at ' + S.W.spheres.get(drag.target).name : 'drag onto a sphere to aim at it')
  }
})

canvas.addEventListener('pointerup', async (e) => {
  const d = drag
  drag = null
  cam.look = false
  if (!d) return
  if (d.kind === 'gimbal') { gimbalCommit(d); return }
  if (d.kind === 'marquee') { endMarquee(d, e.clientX, e.clientY); return }
  if (d.kind === 'look' && d.moved < 4 && performance.now() - d.t < 250) {
    // a short right click removes what is under the cursor
    const p = pick(e.clientX, e.clientY)
    if (p.brick) edit({ op: 'remove', ids: [p.brick] })
    else if (p.datum) edit({ op: 'remove', ids: [p.datum] })
    return
  }
  if (d.kind === 'sphere' && d.to && d.moved > 3 && S.W.spheres.get(d.id).cell) {
    const s = S.W.spheres.get(d.id)
    const delta = [Math.round(d.to.x - d.start.x), 0, Math.round(d.to.z - d.start.z)]
    if (delta[0] || delta[2]) edit({ op: 'move', ids: [s.id], delta })
    else dirty()
    return
  }
  if (d.kind === 'sphere' && d.to && d.moved > 3) {
    const s = S.W.spheres.get(d.id)
    const pos = [d.to.x, d.to.y, d.to.z].map((x) => Math.round(x * 4) / 4)
    const mv = new THREE.Vector3(...pos).sub(d.start)
    const patch = { pos }
    if (e.shiftKey && mv.length() > 0.5) patch.aim = mv.normalize().toArray()
    edit({ op: 'put', t: 'sphere', ...s, ...patch })
  }
  if (d.kind === 'aim' && d.target) {
    const s = S.W.spheres.get(d.id), t = S.W.spheres.get(d.target)
    const a = new THREE.Vector3(...t.pos).sub(new THREE.Vector3(...s.pos)).normalize().toArray()
    edit({ op: 'put', t: 'sphere', ...s, aim: a })
  }
})

canvas.addEventListener('wheel', (e) => {
  e.preventDefault()
  if (walk.on) { if (!walkWheel(e)) cycleSlot(e.deltaY > 0 ? 1 : -1); return }
  if (drag && drag.kind === 'sphere') { drag.depth = Math.max(1, drag.depth * (e.deltaY > 0 ? 1.08 : 0.93)); return }
  if (cam.look && !cam.orbit) { cam.speed = Math.max(1, Math.min(80, cam.speed * (e.deltaY > 0 ? 0.85 : 1.18))); status('speed ' + cam.speed.toFixed(1)); return }
  if (cam.orbit) {
    const off = cam.pos.clone().sub(cam.orbit)
    cam.pos.copy(cam.orbit).addScaledVector(off, e.deltaY > 0 ? 1.1 : 0.9)
    return
  }
  const fwd = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
  cam.pos.addScaledVector(fwd, -Math.sign(e.deltaY) * 1.2)
}, { passive: false })

function hoverAt(x, y) {
  const p = pick(x, y)
  S.hover = p
  if (p.brick) {
    const b = S.W.bricks.get(p.brick)
    hoverRing.visible = true
    hoverRing.position.set(b.cell[0] + 0.5, b.cell[1] + 0.5, b.cell[2] + 0.5)
    $('#hud').textContent = columnLine(b) + '  ·  ' + b.kind + (b.note ? '  ·  ' + b.note : '')
    hoverResolve(b)
  } else hoverRing.visible = false
  if (!ed.open) {
    const c = p.cell
    if (c && placeable(c) && (p.plate || p.brick && (S.armed || false))) {
      ghost.visible = true
      ghost.position.set(c[0] + 0.5, c[1] + 0.5, c[2] + 0.5)
    } else ghost.visible = false
  }
  if (p.datum) {
    const d = datumView(p.datum)
    $('#hud').textContent = datumText(d) + '  ·  tile ' + D.FACE_NAMES[p.face] + ' ' + p.u + ' ' + p.v + ' = bit ' + p.bit + ' (' + ((d.value >> p.bit) & 1) + ')  ·  click selects, click again toggles'
  }
  if (!drag && !walk.on && S.selDatum) {
    const r = canvas.getBoundingClientRect()
    ndc.set(((x - r.left) / r.width) * 2 - 1, -((y - r.top) / r.height) * 2 + 1)
    ray.setFromCamera(ndc, camera)
    const g = gizmo.hit(ray)
    gizmo.active(g)
    if (g) $('#hud').textContent = (g.startsWith('r') ? 'turn about ' + g[1] : g === 's' ? 'scale all three' : 'scale ' + g.slice(1).split('').join(' and ')) + '  ·  Ctrl snaps (1/16, 15°)  ·  type a number for an exact amount  ·  X Y Z constrain'
  }
  if (p.track) $('#hud').textContent = trackText(p.track) + '  ·  Delete or right click removes it'
  if (p.sphere) {
    const s = sphereView(p.sphere)
    $('#hud').textContent = s.kind + ' ' + s.name + (s.shown != null ? '  ·  shows ' + s.shown : '') + '  ·  drag to move, Alt+drag to aim, Shift+drop aims along the move'
  }
}

const typing = () => {
  const a = document.activeElement
  return a && (a.tagName === 'INPUT' || a.tagName === 'TEXTAREA' || a.tagName === 'SELECT')
}

addEventListener('keydown', async (e) => {
  if (e.key === 'F5') { e.preventDefault(); runStart(); return }
  if (e.key === 'F6') { e.preventDefault(); pauseRun(); return }
  if (e.key === 'F9') { e.preventDefault(); step(e.ctrlKey ? 'all' : 'event'); return }
  if (e.key === 'F10') { e.preventDefault(); step(e.shiftKey ? 'column' : 'token'); return }
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') { e.preventDefault(); findIn.focus(); findIn.select(); return }
  if (e.key === '`' && !typing()) { e.preventDefault(); if (dropConsole()) conIn.focus(); else canvas.focus(); return }
  if (typing()) return
  if (drag && drag.kind === 'gimbal') { gimbalKey(e); return }
  if (walk.on && walkKey(e)) return
  if ((e.key === 'f' || e.key === 'F') && !cam.look && !e.ctrlKey && !e.metaKey) { e.preventDefault(); setWalk(true); return }
  if (cam.look) { cam.keys.add(e.code); return }
  const k = e.key
  if (grab.on) {
    if (k === 'Escape') { cancelGrab(); return }
    if (k === 'Enter') { commitGrab(); return }
    if (k === 'PageUp' || k === 'PageDown') { e.preventDefault(); grab.dy += k === 'PageUp' ? 1 : -1; updateGrab(mouse.x, mouse.y); return }
  }
  if (k === '/') { e.preventDefault(); findIn.focus(); findIn.select(); return }
  if (k === 'Tab') { e.preventDefault(); toggleOrbit(); return }
  if ((k === 'g' || k === 'G') && !e.ctrlKey && !e.metaKey) { startGrab(); return }
  if ((k === 't' || k === 'T') && !e.ctrlKey && !e.metaKey) { toggleGimbal(); return }
  if ((k === 'r' || k === 'R') && !e.ctrlKey && !e.metaKey) { rotateSel(e.shiftKey ? 3 : 1); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'z') { e.preventDefault(); e.shiftKey ? redo() : undo(); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'y') { e.preventDefault(); redo(); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'd') { e.preventDefault(); if (S.sel.size) edit({ op: 'duplicate', ids: [...S.sel], delta: [0, 0, 1] }); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'c') { copySel(); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 'v') { paste(); return }
  if ((e.ctrlKey || e.metaKey) && k.toLowerCase() === 's') { e.preventDefault(); status('every edit is saved as it happens'); return }
  if (e.altKey && k.startsWith('Arrow') || e.altKey && (k === 'PageUp' || k === 'PageDown')) {
    e.preventDefault()
    const d = { ArrowLeft: [-1, 0, 0], ArrowRight: [1, 0, 0], ArrowUp: [0, 0, -1], ArrowDown: [0, 0, 1], PageUp: [0, 1, 0], PageDown: [0, -1, 0] }[k]
    const ids = selIds()
    if (ids.length) edit({ op: 'move', ids, delta: d })
    return
  }
  if (k === 'Enter') {
    const id = [...S.sel][0]
    const b = id && S.W.bricks.get(id)
    if (b) openEditor(b.space, b.cell, b.id)
    return
  }
  if (k === 'Delete' || k === 'Backspace') {
    const ids = [...S.sel]
    if (S.selSphere) ids.push(S.selSphere)
    if (S.selDatum) ids.push(S.selDatum)
    if (!ids.length && S.hover && S.hover.track) ids.push(S.hover.track)
    if (ids.length) edit({ op: 'remove', ids })
    return
  }
  if (k === 'Escape') {
    if (S.tool) { setTool(null); return }
    if (!ins.hidden) { ins.hidden = true; return }
    if (S.sel.size || S.selSphere || S.selZone || S.selPlate || S.selDatum) { S.sel.clear(); S.selSphere = S.selZone = S.selPlate = S.selDatum = null; dirty(); return }
    if (leaveSpace()) return
    return
  }
  if (k === ' ') {
    e.preventDefault()
    const b = S.W.bricks.get([...S.sel][0])
    if (b && b.kind === 'box' && !b.word) { enterSpace(b.inner); return }
    if (S.selSphere) { enterSpace(S.W.spheres.get(S.selSphere).body); return }
    return
  }
  if (k === 'e' || k === 'E') {
    const p = S.hover || {}
    if (p.sphere) { S.selSphere = p.sphere; S.sel.clear() }
    else if (p.brick && !S.sel.size) S.sel.add(p.brick)
    else if (!S.selSphere && !S.sel.size) {
      // the nearest sphere to the view center
      let best = null, bd = 14
      for (const s of S.W.spheres.values()) { const d = camera.position.distanceTo(new THREE.Vector3(...s.pos)); if (d < bd) { bd = d; best = s.id } }
      if (best) S.selSphere = best
    }
    dirty()
    openInspector()
    return
  }
  if (k === 'F8') { const b = (S.hover && S.hover.brick) || [...S.sel][0]; if (b) toggleBreak(b); return }
  if (k === 'Home') { frameSpace(); return }
  if (k === 'z' || k === 'Z') { newZone(); return }
  if (k === 'p' || k === 'P') { newPlate(); return }
  if (k === '?') { showKeys(); return }
})
addEventListener('keyup', (e) => { cam.keys.delete(e.code); walk.keys.delete(e.code) })
addEventListener('blur', () => { cam.keys.clear(); walk.keys.clear() })

function moveCamera(dt) {
  if (!cam.look) { cam.keys.clear(); return }
  const v = new THREE.Vector3()
  const K = cam.keys
  if (K.has('KeyW')) v.z -= 1
  if (K.has('KeyS')) v.z += 1
  if (K.has('KeyA')) v.x -= 1
  if (K.has('KeyD')) v.x += 1
  if (K.has('KeyR') || K.has('Space')) v.y += 1
  if (K.has('KeyF') || K.has('KeyC')) v.y -= 1
  const fast = K.has('ShiftLeft') || K.has('ShiftRight') ? 3 : K.has('ControlLeft') ? 0.3 : 1
  if (v.lengthSq()) cam.pos.add(v.normalize().multiplyScalar(cam.speed * fast * dt).applyQuaternion(cam.q))
  let roll = 0
  if (K.has('KeyQ')) roll += 1
  if (K.has('KeyE')) roll -= 1
  if (roll) cam.q.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0, 0, 1), roll * 1.4 * dt))
}

function copySel() {
  const ids = [...S.sel]
  if (!ids.length) return
  const cols = new Map()
  for (const id of ids) { const b = S.W.bricks.get(id); const c = columnOf(b); cols.set(c[0].id, c) }
  const t = [...cols.values()].map((c) => c.map(brickText).join(' ')).join('\n')
  navigator.clipboard && navigator.clipboard.writeText(t).catch(() => {})
  status('copied as s text')
}
async function paste() {
  let t = ''
  try { t = await navigator.clipboard.readText() } catch {}
  if (!t) { status('nothing to paste', true); return }
  const target = ed.open ? ed.cell : S.hover && S.hover.cell
  if (!target) { status('point at a plate to paste', true); return }
  edit({ op: 'paste', space: S.space, cell: target, text: t })
}

async function undo() { const r = await api('undo'); applyChanges(r); liveAfter(r); refreshInspector() }
async function redo() { const r = await api('redo'); applyChanges(r); liveAfter(r); refreshInspector() }

function newZone() {
  if (S.space !== 0) { status('zones live in the world space', true); return }
  const fwd = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
  const c = cam.pos.clone().addScaledVector(fwd, 14)
  const min = [Math.round(c.x) - 4, Math.max(-2, Math.round(c.y) - 4), Math.round(c.z) - 4]
  const n = 'z' + (S.W.zones.size + 1)
  edit({ op: 'zone', name: n, kind: 'async', min, max: [min[0] + 8, min[1] + 8, min[2] + 8] }).then((r) => { if (r && r.id) { S.selZone = r.id; openInspector() } })
}
function newPlate() {
  const fwd = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
  const c = cam.pos.clone().addScaledVector(fwd, 10)
  edit({ op: 'plate', space: S.space, kind: 'main', at: [Math.round(c.x) - 4, Math.round(c.y) - 1, Math.round(c.z) - 3], w: 8, d: 6 }).then((r) => { if (r && r.id) { S.selPlate = r.id; openInspector() } })
}


// ---------- find, box select, grab, rotate, orbit, layers, minimap ----------

const findIn = $('#find'), findN = $('#find-n')
const find = { hits: [], i: -1 }
function runFind() {
  const q = findIn.value.trim().toLowerCase()
  S.found.clear()
  find.hits = []
  find.i = -1
  if (q) {
    for (const b of S.W.bricks.values()) if (brickText(b).toLowerCase().includes(q)) { find.hits.push({ brick: b.id }); S.found.add(b.id) }
    for (const sp of S.W.spheres.values()) if (sp.name.toLowerCase().includes(q)) { find.hits.push({ sphere: sp.id }); S.found.add('s' + sp.id) }
  }
  findN.textContent = q ? find.hits.length + (find.hits.length === 1 ? ' hit' : ' hits') : ''
  dirty()
}
function nextFound(back) {
  if (!find.hits.length) return
  find.i = (find.i + (back ? -1 : 1) + find.hits.length) % find.hits.length
  const hit = find.hits[find.i]
  findN.textContent = find.i + 1 + ' of ' + find.hits.length
  if (hit.brick) {
    const b = S.W.bricks.get(hit.brick)
    if (!b) return
    flyTo(b)
    selectBrick(b.id)
  } else {
    while (S.space !== 0 && leaveSpace()) {}
    const sp = S.W.spheres.get(hit.sphere)
    if (!sp) return
    const c = new THREE.Vector3(...sp.pos)
    const fwd = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
    cam.pos.copy(c).addScaledVector(fwd, -8)
    lookAt(c)
    S.selSphere = sp.id
    S.sel.clear()
    dirty()
    refreshInspector()
  }
}
findIn.addEventListener('input', runFind)
findIn.addEventListener('keydown', (e) => {
  e.stopPropagation()
  if (e.key === 'Enter') { e.preventDefault(); nextFound(e.shiftKey) }
  if (e.key === 'Escape') { findIn.value = ''; runFind(); canvas.focus() }
})

// box select
const marq = $('#marquee')
function showMarquee(d, x, y) {
  marq.hidden = false
  marq.style.left = Math.min(d.x, x) + 'px'
  marq.style.top = Math.min(d.y, y) + 'px'
  marq.style.width = Math.abs(x - d.x) + 'px'
  marq.style.height = Math.abs(y - d.y) + 'px'
}
function endMarquee(d, x, y) {
  marq.hidden = true
  if (Math.abs(x - d.x) + Math.abs(y - d.y) < 6) return
  const [x0, x1] = [Math.min(d.x, x), Math.max(d.x, x)], [y0, y1] = [Math.min(d.y, y), Math.max(d.y, y)]
  if (!d.add) S.sel.clear()
  for (const b of S.W.bricks.values()) {
    if (b.space !== S.space) continue
    const [sx, sy, vis] = cellScreen(b.cell)
    if (vis && sx >= x0 && sx <= x1 && sy >= y0 && sy <= y1) S.sel.add(b.id)
  }
  S.selSphere = S.selZone = S.selPlate = S.selDatum = null
  status(S.sel.size + ' bricks selected')
  dirty()
  refreshInspector()
}

// grab: the selection follows the cursor on its own level of the grid
const grab = { on: false, ids: [], start: null, delta: [0, 0, 0], dy: 0, ghosts: new THREE.Group() }
groups.over.add(grab.ghosts)
function selIds() {
  return S.sel.size ? [...S.sel] : S.selSphere ? [S.selSphere] : S.selDatum ? [S.selDatum] : S.selPlate ? [S.selPlate] : S.selZone ? [S.selZone] : []
}
function levelCell(x, y, level) {
  const r = canvas.getBoundingClientRect()
  ndc.set(((x - r.left) / r.width) * 2 - 1, -((y - r.top) / r.height) * 2 + 1)
  ray.setFromCamera(ndc, camera)
  const p = ray.ray.intersectPlane(new THREE.Plane(new THREE.Vector3(0, 1, 0), -level), new THREE.Vector3())
  return p ? [Math.floor(p.x), Math.floor(p.z)] : null
}
function startGrab() {
  const ids = selIds()
  if (!ids.length) { status('select something to grab', true); return }
  const first = S.W.bricks.get(ids[0]) || null
  const level = first ? first.cell[1] : S.selSphere ? S.W.spheres.get(S.selSphere).pos[1] : 1
  const c = levelCell(mouse.x, mouse.y, level)
  if (!c) return
  Object.assign(grab, { on: true, ids, level, start: c, delta: [0, 0, 0], dy: 0 })
  updateGrab(mouse.x, mouse.y)
}
function updateGrab(x, y) {
  const c = levelCell(x, y, grab.level)
  if (c) grab.delta = [c[0] - grab.start[0], grab.dy, c[1] - grab.start[1]]
  clearGhosts()
  for (const id of grab.ids) {
    const b = S.W.bricks.get(id)
    if (!b) continue
    const g = new THREE.LineSegments(ghost.geometry, ghost.material)
    g.position.set(b.cell[0] + 0.5 + grab.delta[0], b.cell[1] + 0.5 + grab.delta[1], b.cell[2] + 0.5 + grab.delta[2])
    grab.ghosts.add(g)
  }
  status('grab ' + grab.delta.join(' ') + ' · click places · Esc cancels')
}
function clearGhosts() {
  while (grab.ghosts.children.length) grab.ghosts.remove(grab.ghosts.children[0])
}
function endGrab() {
  grab.on = false
  clearGhosts()
}
function commitGrab() {
  const d = grab.delta
  const ids = grab.ids
  endGrab()
  if (d[0] || d[1] || d[2]) edit({ op: 'move', ids, delta: d })
  else status('')
}
function cancelGrab() { endGrab(); status('grab cancelled') }

function rotateSel(turns) {
  const ids = selIds()
  if (!ids.length) { status('select something to turn', true); return }
  const b = S.W.bricks.get(ids[0])
  let pivot
  if (b) {
    // the middle of the selected bricks, so the selection turns in place
    const cs = ids.map((id) => S.W.bricks.get(id)).filter(Boolean).map((x) => x.cell)
    const mid = (i) => Math.floor((Math.min(...cs.map((c) => c[i])) + Math.max(...cs.map((c) => c[i]))) / 2)
    pivot = [mid(0), b.cell[1], mid(2)]
  }
  else if (S.W.spheres.get(ids[0])) { const p = S.W.spheres.get(ids[0]).pos; pivot = [Math.floor(p[0]), Math.floor(p[1]), Math.floor(p[2])] }
  else if (S.W.plates.get(ids[0])) { const p = S.W.plates.get(ids[0]); pivot = [p.at[0] + Math.floor(p.w / 2), p.at[1], p.at[2] + Math.floor(p.d / 2)] }
  else if (S.W.datums.get(ids[0])) pivot = S.W.datums.get(ids[0]).cell
  else { status('zones do not turn', true); return }
  edit({ op: 'rotate', ids, pivot, turns })
}

// orbit around the selection
function selCenter() {
  const pts = []
  for (const id of S.sel) { const b = S.W.bricks.get(id); if (b) pts.push(new THREE.Vector3(b.cell[0] + 0.5, b.cell[1] + 0.5, b.cell[2] + 0.5)) }
  if (S.selSphere) pts.push(new THREE.Vector3(...sphereView(S.selSphere).pos))
  if (!pts.length) return null
  return new THREE.Box3().setFromPoints(pts).getCenter(new THREE.Vector3())
}
function toggleOrbit() {
  if (cam.orbit) { cam.orbit = null; status('fly'); return }
  const c = selCenter() || (S.hover && S.hover.point && S.hover.point.clone()) || cam.pos.clone().addScaledVector(new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q), 10)
  cam.orbit = c
  lookAt(c)
  status('orbit: right mouse turns around the selection, the wheel zooms, Tab flies again')
}
function orbitBy(yaw, pitch) {
  const off = cam.pos.clone().sub(cam.orbit)
  off.applyAxisAngle(new THREE.Vector3(0, 1, 0), yaw)
  const right = new THREE.Vector3(1, 0, 0).applyQuaternion(cam.q)
  const tilted = off.clone().applyAxisAngle(right, pitch)
  if (Math.abs(tilted.clone().normalize().y) < 0.97) off.copy(tilted)
  cam.pos.copy(cam.orbit).add(off)
  lookAt(cam.orbit)
}

// what a word resolves to, after the cursor rests on it
const resolved = new Map()
let resolveTimer = 0
function hoverResolve(b) {
  clearTimeout(resolveTimer)
  const show = (r) => {
    if (!S.hover || S.hover.brick !== b.id || drag) return
    const parts = [brickText(b), r.kind, r.sig, r.doc].filter(Boolean)
    $('#hud').textContent = parts.join('  ·  ')
  }
  if (resolved.has(b.id)) { show(resolved.get(b.id)); return }
  resolveTimer = setTimeout(async () => {
    const r = await api('resolve', { brick: b.id })
    if (r && !r.error) { resolved.set(b.id, r); show(r) }
  }, 300)
}

// layers
function showView() {
  const rows = [['shelves', 'shelf plates and their bricks'], ['links', 'links between spheres'], ['zones', 'zone walls and their labels'], ['labels', 'sphere names (shown values and errors stay)'], ['minimap', 'the top view']]
  const body = h('div', { class: 'layers' }, ...rows.flatMap(([k, d]) => {
    const c = h('input', { type: 'checkbox', onchange: () => { S.layers[k] = c.checked; store.set('layers', S.layers); dirty() } })
    c.checked = S.layers[k]
    return [c, h('span', { text: d })]
  }))
  modal('View', [body])
}

// the minimap: a top view of the current space
const mini = $('#minimap'), mctx = mini.getContext('2d')
let miniAt = 0, miniView = null
function drawMinimap() {
  const mw = $('#minimap-w')
  mw.hidden = !S.layers.minimap
  if (mw.hidden || mw.classList.contains('collapsed')) return
  const pts = []
  for (const p of platesIn(S.space)) pts.push([p.at[0], p.at[2]], [p.at[0] + p.w, p.at[2] + p.d])
  for (const sp of S.W.spheres.values()) if (inSpace(sp)) { const v = sphereView(sp.id); pts.push([v.pos[0], v.pos[2]]) }
  for (const d of S.W.datums.values()) if ((d.space || 0) === S.space) pts.push([d.cell[0], d.cell[2]], [d.cell[0] + 1, d.cell[2] + 1])
  if (S.space === 0) {
    for (const z of S.W.zones.values()) pts.push([z.min[0], z.min[2]], [z.max[0] + 1, z.max[2] + 1])
  }
  pts.push([cam.pos.x, cam.pos.z])
  let [x0, z0, x1, z1] = [Infinity, Infinity, -Infinity, -Infinity]
  for (const [x, z] of pts) { x0 = Math.min(x0, x); x1 = Math.max(x1, x); z0 = Math.min(z0, z); z1 = Math.max(z1, z) }
  const pad = 3
  x0 -= pad; z0 -= pad; x1 += pad; z1 += pad
  const W = mini.width, H = mini.height
  const sc = Math.min(W / (x1 - x0), H / (z1 - z0))
  const ox = (W - (x1 - x0) * sc) / 2, oz = (H - (z1 - z0) * sc) / 2
  const X = (x) => ox + (x - x0) * sc, Z = (z) => oz + (z - z0) * sc
  miniView = { x0, z0, sc, ox, oz }
  mctx.fillStyle = TH().win
  mctx.fillRect(0, 0, W, H)
  for (const p of platesIn(S.space)) {
    if (!S.layers.shelves && p.kind === 'shelf') continue
    mctx.fillStyle = PCOL[p.kind] || PCOL.main
    mctx.fillRect(X(p.at[0]), Z(p.at[2]), p.w * sc, p.d * sc)
  }
  mctx.fillStyle = TH().ink2
  for (const b of S.W.bricks.values()) if (b.space === S.space) mctx.fillRect(X(b.cell[0]), Z(b.cell[2]), Math.max(1, sc), Math.max(1, sc))
  // datums seen from above: their colors, an image
  for (const d of datumItems()) {
    mctx.fillStyle = D.colorOf(d.value)
    const w = Math.max(1, d.parts.sx * sc), dd = Math.max(1, d.parts.sz * sc)
    mctx.fillRect(X(d.cell[0] + 0.5) - w / 2, Z(d.cell[2] + 0.5) - dd / 2, w, dd)
  }
  if (S.space === 0) {
    for (const z of S.W.zones.values()) {
      mctx.strokeStyle = ZCOL[z.kind]
      mctx.strokeRect(X(z.min[0]), Z(z.min[2]), (z.max[0] - z.min[0] + 1) * sc, (z.max[2] - z.min[2] + 1) * sc)
    }
    for (const sp of S.W.spheres.values()) {
      if (!inSpace(sp)) continue
      const v = sphereView(sp.id)
      mctx.fillStyle = SKCOL[v.kind] || SKCOL.fn
      mctx.beginPath()
      mctx.arc(X(v.pos[0]), Z(v.pos[2]), Math.max(2, v.r * sc), 0, Math.PI * 2)
      mctx.fill()
    }
  }
  const f = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
  const a = Math.atan2(f.z, f.x)
  const cx = X(cam.pos.x), cz = Z(cam.pos.z)
  mctx.fillStyle = TH().ink
  mctx.beginPath()
  mctx.moveTo(cx + Math.cos(a) * 9, cz + Math.sin(a) * 9)
  mctx.lineTo(cx + Math.cos(a + 2.5) * 6, cz + Math.sin(a + 2.5) * 6)
  mctx.lineTo(cx + Math.cos(a - 2.5) * 6, cz + Math.sin(a - 2.5) * 6)
  mctx.fill()
}
mini.addEventListener('mousedown', (e) => {
  if (!miniView) return
  const r = mini.getBoundingClientRect()
  const x = (e.clientX - r.left) * (mini.width / r.width), z = (e.clientY - r.top) * (mini.height / r.height)
  const v = miniView
  const wx = v.x0 + (x - v.ox) / v.sc, wz = v.z0 + (z - v.oz) / v.sc
  const f = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
  f.y = 0
  if (f.lengthSq() < 1e-6) f.set(0, 0, -1)
  f.normalize()
  // stand back from the clicked point, looking at it
  cam.pos.set(wx - f.x * 10, cam.pos.y, wz - f.z * 10)
  lookAt(new THREE.Vector3(wx, 1, wz))
  if (cam.orbit) cam.orbit = new THREE.Vector3(wx, 1, wz)
})

// ---------- palette ----------

const TABS = ['code', 'lib', 'graph', 'world']
let tab = store.get('tab', 'code')
async function loadPalette() {
  S.palette = await api('palette')
  for (const g of S.palette.groups) for (const it of g.items) if (it.kind === 'cmd') S.groupOf.set(it.w, g.name)
  renderTabs()
}
function renderTabs() {
  const t = $('#tabs')
  t.innerHTML = ''
  for (const n of TABS) t.append(h('button', { class: n === tab ? 'on' : '', text: n, onclick: () => { tab = n; store.set('tab', n); renderTabs() } }))
  renderPalette()
}
function useWord(w) {
  if (ed.open) { edIn.value = w; edIn.focus(); complete(); return }
  const id = [...S.sel][0]
  if (id && S.sel.size === 1) { edit({ op: 'set', id, word: w }); return }
  S.armed = w
  status('click a plate cell to place ' + w + ' (Alt+click on a brick face stacks onto it)')
}
function setTool(k) {
  S.tool = k
  for (const el of document.querySelectorAll('.tile')) el.classList.toggle('on', el.dataset.kind === k || (el.dataset.kind === 'gimbal' && S.gimbal))
  if (k === 'wire') status('wire: click the sphere or device it starts at')
  else if (k && k.startsWith('asm:')) status('click in the world to place ' + k.slice(4) + ' (Shift keeps the tool)')
  else if (k === 'track') status('click the cell the track starts at, then where it ends: it runs along x, then z')
  else if (k && k.startsWith('machine:')) status('click the cell past a track\'s end for the ' + k.slice(8) + ' (Shift keeps the tool)')
  else if (k === 'datum') status('click the ground, a plate or a cube face to set a datum cube there (Shift keeps the tool)')
  else status(k ? 'click in the world to place a ' + k + (S.gridPlace || k === 'mast' ? ' device on the grid' : ' sphere') + ' (Shift keeps the tool)' : '')
}
async function renderPalette() {
  const box = $('#pal-items')
  box.innerHTML = ''
  const f = $('#pal-filter').value.trim().toLowerCase()
  const item = (it, cls) => h('div', { class: 'item', title: [it.sig, it.doc].filter(Boolean).join(' — '), onclick: () => useWord(it.w) }, h('code', { class: 'k-' + cls, text: it.w }), h('span', { text: it.sig || it.doc || '' }))
  const match = (it) => !f || it.w.toLowerCase().includes(f) || (it.doc || '').toLowerCase().includes(f)
  if (!S.palette) return
  if (tab === 'code') {
    for (const g of S.palette.groups.filter((g) => !g.name.startsWith('std/') && g.name !== 'graph')) {
      const items = g.items.filter(match)
      if (!items.length) continue
      box.append(h('div', { class: 'note', text: g.name }))
      for (const it of items) box.append(item(it, 'cmd'))
    }
    box.append(h('div', { class: 'note', text: 'caps' }))
    for (const c of S.palette.caps) box.append(h('div', { class: 'item', title: c.doc, onclick: () => useWord('name' + c.sigil) }, h('code', { class: 'k-look', text: 'x' + c.sigil }), h('span', { text: c.name + ' — ' + c.doc })))
    box.append(h('div', { class: 'note', text: 'literals' }))
    for (const w of ['{}', "'text'", '{~x~}']) box.append(item({ w, doc: w === '{}' ? 'box: a quotation with an inside' : w === '{~x~}' ? 'the value of x as data' : 'quote: raw text' }, 'brace'))
  } else if (tab === 'lib') {
    for (const g of S.palette.groups.filter((g) => g.name.startsWith('std/'))) {
      const items = g.items.filter(match)
      if (!items.length) continue
      box.append(h('div', { class: 'note', text: g.name + ' (load with ' + g.name + '.)' }))
      for (const it of items) box.append(item(it, 'lib'))
    }
  } else if (tab === 'graph') {
    box.append(h('div', { class: 'note', text: 'spheres and devices: click, then click in the world' }))
    const gp = h('input', { type: 'checkbox', onchange: () => { S.gridPlace = gp.checked; store.set('gridPlace', gp.checked); dirty() } })
    gp.checked = S.gridPlace
    box.append(h('label', { class: 'item' }, gp, h('span', { text: 'place on the grid as devices' })))
    const kinds = ['src', 'fn', 'ratchet', 'out', 'gate', 'mast']
    if (S.W.spaces.get(S.space) && S.W.spaces.get(S.space).owner === 'assembly') kinds.push('port')
    for (const k of kinds) {
      const d = { src: 'source: starts pulses', fn: 'function: runs on the arriving line', ratchet: 'ratchet: loops n times, then exit', out: 'output: shows the line', gate: 'gate: sits on a zone wall', mast: 'mast: a pole from a grid cell up to the sphere layer', port: 'port: where lines enter or leave this assembly' }[k]
      box.append(h('div', { class: 'tile', 'data-kind': k, onclick: () => setTool(S.tool === k ? null : k) }, h('span', { class: 'orb', style: 'background:' + SKCOL[k] }), h('span', { text: d })))
    }
    box.append(h('div', { class: 'tile', 'data-kind': 'wire', onclick: () => { S.wireFrom = null; setTool(S.tool === 'wire' ? null : 'wire') } }, h('span', { class: 'orb', style: 'background:#6d5a47;border-radius:0;height:4px' }), h('span', { text: 'wire: click one sphere, then another' })))
    box.append(h('div', { class: 'tile', 'data-kind': 'datum', onclick: () => setTool(S.tool === 'datum' ? null : 'datum') }, h('span', { class: 'orb', style: 'background:linear-gradient(90deg,#161616 50%,#f4f4f4 50%);border-radius:0' }), h('span', { text: 'datum: a cube of 24 bits, red green blue' })))
    box.append(h('div', { class: 'tile' + (S.gimbal ? ' on' : ''), 'data-kind': 'gimbal', onclick: toggleGimbal }, h('span', { class: 'orb', style: 'background:conic-gradient(#d9534f 0 33%,#4caf50 0 66%,#4a7bd0 0)' }), h('span', { text: 'gimbal (T): scale and turn the selected cube' })))
    box.append(h('div', { class: 'tile', 'data-kind': 'switch', onclick: () => setTool(S.tool === 'switch' ? null : 'switch') }, h('span', { class: 'orb', style: 'background:' + SKCOL.src + ';border-radius:0' }), h('span', { text: 'switch: a source that puts out 0 or 1' })))
    box.append(h('div', { class: 'note', text: 'tracks and machines: a track runs from the first cell clicked to the second; a machine past its end takes what it carries' }))
    box.append(h('div', { class: 'tile', 'data-kind': 'track', onclick: () => { S.trackFrom = null; setTool(S.tool === 'track' ? null : 'track') } }, h('span', { class: 'orb', style: 'background:repeating-linear-gradient(90deg,#3d3530 0 5px,#8a7a64 5px 8px);border-radius:0' }), h('span', { text: 'track: carries items, a cell a tick' })))
    for (const m of MACHINES) box.append(h('div', { class: 'tile', 'data-kind': 'machine:' + m, onclick: () => setTool(S.tool === 'machine:' + m ? null : 'machine:' + m) }, h('span', { class: 'orb', style: 'background:' + MCOL[m] + ';border-radius:2px' }), h('span', { text: m + ': ' + MACHINE_TEXT[m] })))
    box.append(h('div', { class: 'note', text: 'scopes: pick a dialect, then click two corners on the ground' }))
    for (const d of DIALECTS) box.append(h('div', { class: 'tile', 'data-kind': 'scope:' + d, onclick: () => { S.scopeFrom = null; setTool(S.tool === 'scope:' + d ? null : 'scope:' + d) } }, h('span', { class: 'orb', style: 'background:' + DCOL[d] + ';border-radius:0' }), h('span', { text: d + (d === 'logic' ? ': gates on bits, by ticks' : d === 's' ? ': plain s, words of its own' : ': ' + d + ' words') })))
    const asms = [...S.W.spaces.values()].filter((x) => x.owner === 'assembly')
    if (asms.length) {
      box.append(h('div', { class: 'note', text: 'assemblies: click, then place' }))
      for (const a of asms) box.append(h('div', { class: 'tile', 'data-kind': 'asm:' + a.name, onclick: () => setTool(S.tool === 'asm:' + a.name ? null : 'asm:' + a.name) }, h('span', { class: 'orb', style: 'background:' + SKCOL.asm + ';border-radius:0' }), h('span', { text: a.name })))
    }
    box.append(h('div', { class: 'note', text: 'plates and zones' }))
    box.append(h('div', { class: 'tile', onclick: newPlate }, h('span', { text: 'new plate (P)' })))
    box.append(h('div', { class: 'tile', onclick: newZone }, h('span', { text: 'new zone (Z)' })))
    const g = S.palette.groups.find((g) => g.name === 'graph')
    box.append(h('div', { class: 'note', text: 'graph words' }))
    for (const it of g.items.filter(match)) box.append(item(it, 'geo'))
  } else if (tab === 'world') {
    const r = await api('complete', { prefix: '' })
    const items = (r.items || []).filter((x) => ['def', 'call', 'look'].includes(x.kind)).filter(match)
    if (!items.length) box.append(h('div', { class: 'note', text: 'Words this world defines show here: shelf definitions, scripts, spheres, names.' }))
    for (const it of items) box.append(item(it, it.kind))
  }
}
$('#pal-filter').addEventListener('input', renderPalette)

// ---------- running ----------

function readRuntime(rep) {
  const rt = { spheres: new Map(), links: [], zones: new Map(), datums: new Map(), tracks: new Map(), machines: new Map() }
  for (const z of rep.zones || []) {
    rt.zones.set(z.name, z)
    for (const d of z.datums || []) rt.datums.set(d.id, d)
    const g = z.graph
    if (!g) continue
    for (const t of g.tracks || []) rt.tracks.set(t.id, t.items)
    for (const m of g.machines || []) rt.machines.set(m.id, m)
    for (const s of g.spheres) {
      const prev = rt.spheres.get(s.id)
      if (!prev || s.shown != null || s.error) rt.spheres.set(s.id, s)
    }
    for (const l of g.links) if (!rt.links.some((x) => x.id === l.id)) rt.links.push(l)
  }
  return rt
}

function showReport(rep) {
  if (rep.problems) { for (const p of rep.problems) log('problem: ' + p.msg, 'err'); status('the world does not build', true); return }
  if (rep.error) { status(rep.error, true); return }
  if (rep.out) $('#out').append(document.createTextNode(rep.out)), ($('#out').scrollTop = 1e9)
  for (const e of rep.errors || []) {
    const el = log('error' + (e.zone && e.zone !== 'main' ? ' in ' + e.zone : '') + ': ' + e.msg, 'err')
    if (e.brick) { S.errBricks.add(e.brick); el.onclick = () => { flyTo(S.W.bricks.get(e.brick)); selectBrick(e.brick) } }
    else if (e.sphere) el.onclick = () => { S.selSphere = e.sphere; openInspector() }
  }
  if ($('#c-trace').checked) for (const t of rep.trace || []) {
    if (t.s) {
      const s = S.W.spheres.get(t.s)
      log('#' + t.n + ' ' + (s ? s.name : t.s) + ' [' + t.in.join(' ') + '] → [' + t.out.join(' ') + ']' + (t.port !== 'out' ? ' ' + t.port : '') + (t.err ? ' ✗ ' + t.err : ''), 'tr')
    } else if (t.spawn) log('zone ' + t.zone + ' spawned', 'tr')
    else if (t.joined) log('zone ' + t.zone + ' joined', 'tr')
    else if (t.halt) log('budget reached in ' + t.zone + ': ' + t.halt + ' events dropped', 'err')
  }
  for (const t of rep.trace || []) if (t.via) pulse(t.via)
  const wasMachines = S.rt && S.rt.machines
  S.rt = readRuntime(rep)
  itemsArrived()
  if (wasMachines && JSON.stringify([...wasMachines]) !== JSON.stringify([...S.rt.machines])) dirty()
  const main = (rep.zones || [])[0] || {}
  S.lastMain = main
  S.cur = main.brick || null
  showStack(main)
  const zs = (rep.zones || []).map((z) => z.name + ':' + z.phase).join(' ')
  $('#hud').textContent = zs + (main.depth ? '  ·  depth ' + main.depth : '') + (main.ow ? '  ·  ow' : '')
  dirty()
}

function showStack(z) {
  const st = $('#stack')
  st.innerHTML = ''
  if (!S.run || !z || !z.line || !S.cur) return
  const b = S.W.bricks.get(S.cur)
  if (!b || b.space !== S.space) return
  const [x, y, vis] = cellScreen([b.cell[0] + 1, b.cell[1], b.cell[2]])
  if (!vis) return
  st.style.left = x + 'px'
  st.style.top = 'auto'
  st.style.bottom = (innerHeight - y - 10) + 'px'
  for (const f of z.frames || []) st.append(h('div', { class: 'v frame', text: '( ' + f.join(' ') }))
  for (const v of z.line) st.append(h('div', { class: 'v', text: v }))
  if (z.ow) st.append(h('div', { class: 'v ow', text: 'ow' }))
}

const dots = []
function pulse(lid) {
  const l = (S.rt ? S.rt.links : []).find((x) => x.id === lid) || S.W.links.get(lid)
  if (!l) return
  const m = new THREE.Mesh(sphereGeo, new THREE.MeshBasicMaterial({ color: 0x4a3322 }))
  m.scale.setScalar(0.2)
  groups.over.add(m)
  dots.push({ m, l, t: 0 })
}
function updateDots(dt) {
  for (let i = dots.length - 1; i >= 0; i--) {
    const d = dots[i]
    d.t += dt / 0.45
    const a = sphereView(d.l.from), b = sphereView(d.l.to)
    if (d.t >= 1 || !a || !b) { groups.over.remove(d.m); d.m.material.dispose(); dots.splice(i, 1); continue }
    d.m.position.set(...a.pos).lerp(new THREE.Vector3(...b.pos), d.t)
  }
}

let loopTimer = 0
async function runStart() {
  if (S.run && !S.run.done) { startLoop(); return }
  if (S.run) await stopRun()
  S.errBricks.clear()
  $('#out').innerHTML = ''
  if (S.W.tracks.size) $('#c-clock').checked = true
  const rep = await api('run', { lockstep: $('#c-lock').checked, breaks: [...S.breaks] })
  if (rep.problems || rep.error) { showReport(rep); return }
  tutorialEvent('run')
  S.run = { paused: false, done: false }
  setRunButtons()
  showReport(rep)
  startLoop()
}
function startLoop() {
  if (!S.run) return
  S.run.paused = false
  S.run.done = false
  setRunButtons()
  clearTimeout(loopTimer)
  const tickLoop = async () => {
    if (!S.run || S.run.paused) return
    const rep = await api('poll', { ops: 400000, ticks: $('#c-clock').checked ? 1 : 0 })
    if (!S.run) return
    showReport(rep)
    if (rep.stop === 'break') { S.run.paused = true; status('breakpoint'); setRunButtons(); return }
    if (rep.stop === 'exit') { status('exited with ' + rep.exit); S.run.paused = true; setRunButtons(); return }
    if (rep.idle && !$('#c-clock').checked) { status(rep.halted ? 'stopped at the budget' : 'done'); S.run.paused = true; S.run.done = true; setRunButtons(); return }
    loopTimer = setTimeout(tickLoop, $('#c-clock').checked ? 100 : 30)
  }
  tickLoop()
}
function pauseRun() { if (S.run) { S.run.paused = true; setRunButtons(); status('paused') } }
async function step(kind) {
  if (!S.run) {
    S.errBricks.clear()
    $('#out').innerHTML = ''
    const rep = await api('run', { lockstep: $('#c-lock').checked, breaks: [...S.breaks] })
    if (rep.problems || rep.error) { showReport(rep); return }
    S.run = { paused: true }
    showReport(rep)
  }
  S.run.paused = true
  setRunButtons()
  const rep = await api('step', { kind, ops: kind === 'all' ? 400000 : 1 })
  showReport(rep)
  status(rep.stop === 'idle' ? 'done' : 'stepped ' + kind)
  const b = S.cur && S.W.bricks.get(S.cur)
  if (b && b.space !== S.space) enterSpace(b.space)
}
async function stopRun(why) {
  if (!S.run) return
  endRunHere(why)
  await api('stop')
}
function endRunHere(why) {
  clearTimeout(loopTimer)
  S.run = null
  S.rt = null
  S.cur = null
  clearItems()
  $('#stack').innerHTML = ''
  setRunButtons()
  if (why) status(why, /ended/.test(why))
  dirty()
}
function setRunButtons() {
  const running = !!S.run && !S.run.paused
  $('#b-run').classList.toggle('on', running)
  $('#b-pause').disabled = !running
  $('#b-stop').disabled = !S.run
}
async function toggleBreak(id) {
  const on = !S.breaks.has(id)
  on ? S.breaks.add(id) : S.breaks.delete(id)
  if (S.run) await api('break', { brick: id, on })
  dirty()
  refreshInspector()
}

$('#b-run').onclick = runStart
$('#b-pause').onclick = pauseRun
$('#b-token').onclick = () => step('token')
$('#b-column').onclick = () => step('column')
$('#b-event').onclick = () => step('event')
$('#b-stop').onclick = () => stopRun('stopped')
$('#b-undo').onclick = undo
$('#b-redo').onclick = redo
$('#b-clear').onclick = () => { $('#out').innerHTML = '' }

// ---------- files, text, build, levels ----------

function modal(title, body, buttons = []) {
  $('#m-title').textContent = title
  const b = $('#m-body')
  b.innerHTML = ''
  b.append(...[body].flat())
  const row = $('#m-buttons')
  row.innerHTML = ''
  for (const [t, f] of [...buttons, ['Close', () => {}]]) row.append(h('button', { class: 'btn', text: t, onclick: async () => { const keep = await f(); if (!keep) $('#modal').hidden = true } }))
  $('#modal').hidden = false
}
$('#modal').addEventListener('mousedown', (e) => { if (e.target.id === 'modal') $('#modal').hidden = true })

function download(name, text, type = 'text/plain') {
  const a = h('a', { href: URL.createObjectURL(new Blob([text], { type })), download: name })
  document.body.append(a)
  a.click()
  a.remove()
}

$('#b-text').onclick = () => showText('world')
async function showText(which) {
  const tabs = h('div', { class: 'row' },
    h('button', { class: 'btn' + (which === 'world' ? ' on' : ''), text: 'World as text', onclick: () => showText('world') }),
    h('button', { class: 'btn' + (which === 'program' ? ' on' : ''), text: 'Program per zone', onclick: () => showText('program') }))
  if (which === 'program') {
    const r = await api('text')
    if (r.problems) { modal('Text', [tabs, ...r.problems.map((p) => h('div', { class: 'err', text: p.msg }))]); return }
    const body = r.zones.map((z) => [h('b', { text: z.name === 'main' ? S.name + '.s' : S.name + '.' + z.name + '.s' }), h('pre', { text: z.text })])
    modal('Text', [tabs, ...body.flat()], [['Download main .s', () => download(S.name + '.s', r.zones[0].text)]])
    return
  }
  const r = await api('worldtext')
  const ta = h('textarea', { class: 'code', spellcheck: 'false', style: 'min-height: 50vh' })
  ta.value = r.text || ''
  ta.addEventListener('keydown', (e) => e.stopPropagation())
  modal('Text', [tabs, h('div', { class: 'note', text: 'The whole world as s: plates with their columns, spheres and devices with their bodies, wires, beams, zones and assemblies. Apply rebuilds the world from it, as one undo step.' }), ta], [
    ['Apply', async () => {
      const c = await api('fromtext', { text: ta.value })
      if (c.error) { status(c.error, true); return true }
      loadWorld(c.world)
      S.undo = c.undo; S.redo = c.redo
      syncUndo()
      liveAfter(c)
      status('the world was rebuilt from its text')
    }],
    ['Download .world.s', () => { download(S.name + '.world.s', ta.value); return true }],
  ])
}

$('#b-build').onclick = async () => {
  const r = await api('build')
  if (r.problems || r.error) { showReport(r); return }
  log('building ' + S.name + ' with rustc…', 'tr')
  const poll = async () => {
    const j = await api('job', { job: r.job })
    if (!j.done) { setTimeout(poll, 600); return }
    if (j.log) log(j.log.trim(), j.ok ? 'tr' : 'err')
    if (j.out) $('#out').append(document.createTextNode(j.out))
    log(j.ok ? 'compiled program finished' : 'build or run failed', j.ok ? 'ok' : 'err')
  }
  poll()
}

function showFile() {
  const fileIn = h('input', { type: 'file', accept: '.s,.txt,.json' })
  const nameIn = h('input', { placeholder: 'new world name' })
  modal('File', [
    h('div', { class: 'note', text: 'Import an s program as a new world (each line becomes a column), or a world .json file.' }),
    h('div', { class: 'row' }, fileIn, nameIn),
  ], [
    ['Import', async () => {
      const f = fileIn.files[0]
      if (!f) return true
      const text = await f.text()
      const name = (nameIn.value || f.name.replace(/\.[^.]+$/, '')).replace(/[^A-Za-z0-9_-]/g, '_')
      if (f.name.endsWith('.json')) {
        status('copy the .json into the worlds folder to open it')
        return
      }
      const r = await api('import', { name, text })
      if (r.error) { status(r.error, true); return true }
      await openWorld(r.name)
    }],
    ['Export world .json', async () => { const r = await api('world'); download(S.name + '.json', r.text, 'application/json') }],
    ['Export .rs', async () => { const r = await api('compile'); if (r.rs) download(S.name + '.rs', r.rs); else showReport(r) }],
  ])
}

function newWorldDialog() {
  const inp = h('input', { placeholder: 'name: letters, digits, _ and -' })
  modal('New world', [inp], [['Create', async () => { const n = inp.value.trim(); if (!/^[A-Za-z0-9_-]+$/.test(n)) return true; await openWorld(n) }]])
  setTimeout(() => inp.focus(), 10)
}


async function checkGoal() {
  const r = await api('check')
  const lv = S.name.startsWith('level-') ? S.name.slice(6) : null
  if (r.ok && lv) { const st = store.get('stars', {}); st[lv] = Math.max(st[lv] || 0, r.stars); store.set('stars', st) }
  if (r.ok) tutorialEvent('solved')
  modal(r.ok ? 'Solved ' + '★'.repeat(r.stars) + '☆'.repeat(3 - r.stars) : 'Not yet', [
    h('div', { class: 'doc', text: r.ok ? 'The goal holds.' : r.why }),
    h('div', { class: 'note', text: r.bricks + ' bricks · ' + r.spheres + ' spheres · ' + r.events + ' pulse events' }),
    r.output ? h('pre', { text: r.output }) : null,
  ].filter(Boolean))
}

function showKeys() {
  const rows = [
    ['hold right mouse', 'fly: W A S D, R F up and down, Q E roll, Shift fast, wheel sets speed'],
    ['short right click', 'remove the brick under the cursor'],
    ['middle drag, wheel', 'pan, move forward and back'],
    ['click a plate cell', 'type a word there; Enter places it and moves up the column'],
    ['Ctrl+Enter (typing)', 'place and start the next column along the arrow'],
    ['Tab, ↑ ↓ (typing)', 'take a completion'],
    ['click a brick', 'select it; Shift+click adds; Alt+click stacks a new brick on that face'],
    ['double-click a brick', 'edit its whole column as one line'],
    ['Enter', 'edit the selected brick'],
    ['Delete', 'remove the selection'],
    ['Alt+arrows, Alt+PgUp/PgDn', 'move the selection'],
    ['G', 'grab the selection: it follows the cursor on the grid; click places, PgUp/PgDn lift, Esc cancels'],
    ['R, Shift+R', 'turn the selection a quarter about +y, counterclockwise or clockwise'],
    ['drag from empty space, Ctrl+drag', 'box select bricks; Shift with Ctrl adds'],
    ['Tab', 'orbit: the right mouse turns around the selection, the wheel zooms'],
    ['Ctrl+F or /', 'find words and spheres; Enter goes to the next'],
    ['Ctrl+D, Ctrl+C, Ctrl+V', 'duplicate, copy as s text, paste text as columns'],
    ['Ctrl+Z, Ctrl+Shift+Z', 'undo, redo'],
    ['Space', 'open a box or a sphere body as its own room; Esc leaves'],
    ['E', 'inspector for what is under the cursor, or the nearest sphere'],
    ['drag a sphere', 'move it; wheel while dragging pushes it nearer or further; Shift on drop aims it along the move'],
    ['Alt+drag a sphere', 'aim it at another sphere: the strike links them'],
    ['Shift+click a plate', 'plate settings: kind, name, size, arrow'],
    ['P, Z', 'new plate, new zone in front of you'],
    ['F5, F6', 'run, pause'],
    ['F10, Shift+F10, F9', 'step a token, a column, a pulse event'],
    ['F8', 'breakpoint on the brick'],
    ['F', 'walk or fly. Walking: W A S D, mouse looks, Space jumps and vaults a ledge, Shift sprints, Ctrl crouches, C slides from a sprint'],
    ['walking: wheel, middle button, 1–9', 'pick from the bar on the right; left click uses it, right click turns a device or removes a brick'],
    ['walking: E, Q, R, Enter', 'carry what you aim at and set it down, drop it here, ride the link you aim at, set the brick in hand'],
    ['`', 'drop the console over the scene; Enter runs s'],
    ['≡', 'the menu: worlds, tutorial, theme, layout'],
    ['drag a strip, double-click it', 'move a window, fold it; docked at the bottom it flips'],
    ['edit while running', 'each zone takes it at its next event; adding or changing a zone ends the run'],
    ['Home', 'frame the space'],
  ]
  modal('Keys', h('dl', { class: 'keys' }, ...rows.flatMap(([k, d]) => [h('dt', { text: k }), h('dd', { text: d })])))
}
$('#b-check').onclick = checkGoal

async function openWorld(name, level) {
  await stopRun()
  S.name = name
  store.set('world', name)
  const r = await api('open', level ? { level } : {})
  if (r.error) { status(r.error, true); return }
  S.undo = r.undo; S.redo = r.redo
  syncUndo()
  S.space = 0
  spaceStack.length = 0
  S.sel.clear()
  S.selSphere = S.selZone = S.selPlate = S.selDatum = null
  S.breaks.clear()
  loadWorld(r.world)
  build()
  frameSpace()
  await refreshWorldList()
  $('#b-check').hidden = !S.W.goal
  if (S.W.goal && !tut.on) { ins.hidden = false; refreshInspector() }
  guideOpen()
  status('opened ' + name)
  renderPalette()
}
async function refreshWorldList() {
  const r = await api('list')
  const sel = $('#world-pick')
  sel.innerHTML = ''
  const names = new Set([...(r.worlds || []), S.name])
  for (const n of [...names].sort()) sel.append(h('option', { value: n, text: n }))
  sel.value = S.name
}
$('#world-pick').onchange = (e) => openWorld(e.target.value)



// ---------- walking: first person, crosshair, hotbar, carrying, riding ----------

const walk = { on: false, p: null, yaw: 0, pitch: 0, keys: new Set(), locked: false, carry: null, carryDatum: null, ride: null, slot: 0, word: store.get('walk-word', ''), center: null, gaxis: '', gim: null }
const SLOT_DEFAULT = [{ k: 'hand' }, { k: 'brick' }, { k: 'wire' }, { k: 'src' }, { k: 'fn' }, { k: 'out' }, { k: 'datum' }, { k: 'gimbal' }, { k: 'inspect' }, { k: 'remove' }, { k: 'ratchet' }, { k: 'mast' }]
let slots = store.get('hotbar', SLOT_DEFAULT)
const SLOT_TEXT = { track: 'track', hand: 'hand', brick: 'brick', wire: 'wire', src: 'source', fn: 'function', out: 'output', ratchet: 'ratchet', mast: 'mast', gate: 'gate', datum: 'datum cube', gimbal: 'gimbal', inspect: 'inspect', remove: 'remove' }
const SLOT_SWATCH = { datum: 'background:linear-gradient(90deg,#161616 50%,#f4f4f4 50%)', gimbal: 'background:conic-gradient(#d9534f 0 33%,#4caf50 0 66%,#4a7bd0 0)' }

// what stands in a cell of the current space: bricks, devices, plate slabs, the ground
let devIndex = new Map()
function reindexDevices() {
  devIndex = new Map()
  for (const sp of S.W.spheres.values()) if (inSpace(sp) && sp.cell && sp.id !== walk.carry) devIndex.set(cellKey(S.space, sp.cell), sp.id)
  // a cube fills the cells whose centers it holds, and always its own: scaled cubes are walls and floors
  datumCells = new Set()
  for (const d of datumItems()) {
    if (d.id === walk.carryDatum) continue
    datumCells.add(cellKey(S.space, d.cell))
    const cs = []
    for (const x of [-0.5, 0.5]) for (const y of [0, 1]) for (const z of [-0.5, 0.5]) cs.push(D.toWorld(d.cell, d.parts, [x, y, z]))
    const lo = [0, 1, 2].map((i) => Math.floor(Math.min(...cs.map((c) => c[i])))), hi = [0, 1, 2].map((i) => Math.floor(Math.max(...cs.map((c) => c[i]))))
    if ((hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1) > 8192) continue
    for (let x = lo[0]; x <= hi[0]; x++) for (let y = lo[1]; y <= hi[1]; y++) for (let z = lo[2]; z <= hi[2]; z++) {
      const l = D.toLocal(d.cell, d.parts, [x + 0.5, y + 0.5, z + 0.5])
      if (Math.abs(l[0]) <= 0.5 && l[1] >= 0 && l[1] <= 1 && Math.abs(l[2]) <= 0.5) datumCells.add(cellKey(S.space, [x, y, z]))
    }
  }
}
function solidAt(x, y, z) {
  if (y <= 0) return true
  const k = cellKey(S.space, [x, y, z])
  if (cellIndex.has(k) || devIndex.has(k) || datumCells.has(k)) return true
  for (const p of platesIn(S.space)) if (p.at[1] === y && x >= p.at[0] && x < p.at[0] + p.w && z >= p.at[2] && z < p.at[2] + p.d) return true
  return false
}

function setWalk(on) {
  if (on === walk.on) return
  walk.on = on
  document.body.classList.toggle('walking', on)
  $('#crosshair').hidden = !on
  $('#hotbar').hidden = !on
  if (on) {
    reindexDevices()
    const e = new THREE.Euler().setFromQuaternion(cam.q, 'YXZ')
    walk.yaw = e.y
    walk.pitch = Math.max(-1.5, Math.min(1.5, e.x))
    walk.p = newPlayer([cam.pos.x, Math.max(1, cam.pos.y - WALK.eye), cam.pos.z])
    while (playerHits(walk.p.feet, WALK.h, solidAt)) walk.p.feet[1] += 1
    cam.orbit = null
    closeEditor()
    renderHotbar()
    lockPointer()
    status('walk: WASD, Space jumps, Shift sprints, Ctrl crouches, C slides; wheel picks from the bar; E carries; R rides a link; F flies')
  } else {
    walk.ride = null
    dropCarry(true)
    if (document.pointerLockElement === canvas) document.exitPointerLock()
    status('fly')
  }
}
function lockPointer() {
  if (!walk.on || document.pointerLockElement === canvas) return
  try { const r = canvas.requestPointerLock(); if (r && r.catch) r.catch(() => {}) } catch {}
}
document.addEventListener('pointerlockchange', () => {
  const was = walk.locked
  walk.locked = document.pointerLockElement === canvas
  if (walk.on && was && !walk.locked && !walk.keepFree) { status('mouse mode: click the view to walk on'); showMenu() }
  walk.keepFree = false
})
addEventListener('mousemove', (e) => {
  if (!walk.on || !walk.locked || walk.ride) return
  walk.yaw -= e.movementX * 0.0022
  walk.pitch = Math.max(-1.55, Math.min(1.55, walk.pitch - e.movementY * 0.0022))
})

function walkInput() {
  const K = walk.keys
  return {
    fwd: (K.has('KeyW') ? 1 : 0) - (K.has('KeyS') ? 1 : 0),
    right: (K.has('KeyD') ? 1 : 0) - (K.has('KeyA') ? 1 : 0),
    yaw: walk.yaw,
    jump: K.has('Space'),
    sprint: K.has('ShiftLeft') || K.has('ShiftRight'),
    crouch: K.has('ControlLeft') || K.has('ControlRight'),
    slide: K.has('KeyC'),
  }
}

function walkFrame(dt) {
  if (needBuild) build()
  if (walk.ride) { rideFrame(dt); return }
  const inp = walkInput()
  let t = dt
  while (t > 1e-6) {
    const h = Math.min(t, 1 / 120)
    stepPlayer(walk.p, inp, h, solidAt)
    t -= h
  }
  // falling out of the world brings the player back to the origin
  if (walk.p.feet[1] < -60) walk.p = newPlayer([0.5, 3, 0.5])
  const f = walk.p.feet
  cam.pos.set(f[0], f[1] + eyeHeight(walk.p), f[2])
  cam.q.setFromEuler(new THREE.Euler(walk.pitch, walk.yaw, 0, 'YXZ'))
  aimFrame()
}

// what the crosshair points at, with the cell an equipped thing would go to
function centerPick() {
  const r = canvas.getBoundingClientRect()
  return pick(r.left + r.width / 2, r.top + r.height / 2)
}
function aimFrame() {
  camera.position.copy(cam.pos)
  camera.quaternion.copy(cam.q)
  camera.updateMatrixWorld()
  const p = centerPick()
  walk.center = p
  const sl = slots[walk.slot] || {}
  const places = ['brick', 'src', 'fn', 'out', 'ratchet', 'mast', 'gate', 'datum', 'track'].includes(sl.k) || /^(asm|machine):/.test(sl.k || '')
  const cell = places && p.cell && !p.sphere ? p.cell : null
  ghost.visible = !!cell && !brickAt(S.space, cell) && !devIndex.has(cellKey(S.space, cell)) && !datumAt(S.space, cell)
  if (cell) ghost.position.set(cell[0] + 0.5, cell[1] + 0.5, cell[2] + 0.5)
  hoverRing.visible = !!p.brick
  if (p.brick) { const b = S.W.bricks.get(p.brick); hoverRing.position.set(b.cell[0] + 0.5, b.cell[1] + 0.5, b.cell[2] + 0.5) }
  const what = p.sphere ? S.W.spheres.get(p.sphere).kind + ' ' + S.W.spheres.get(p.sphere).name : p.brick ? columnLine(S.W.bricks.get(p.brick)) : p.datum ? datumText(datumView(p.datum)) + '  ·  bit ' + p.bit : p.track ? trackText(p.track) : p.plate ? 'plate' : p.ground ? 'ground' : ''
  $('#hud').textContent = (SLOT_TEXT[sl.k] || (sl.k || '').replace('asm:', 'assembly ').replace('machine:', '')) + (sl.k === 'brick' ? ' ' + (walk.word || '(Enter sets the word)') : '') + (sl.k === 'gimbal' ? ' ' + (walk.gaxis ? 'along ' + walk.gaxis : 'all axes') + ' (X Y Z pick, again for all) · wheel scales, Shift+wheel turns' : '') + (what ? '  ·  ' + what : '')
  if (walk.carryDatum) buildDatums()
  // what is carried floats in front
  if (walk.carry) {
    const g = groups.graph.children.find((c) => c.children.some((x) => x.userData.sphere === walk.carry))
    if (g) g.position.copy(holdPoint())
  }
}
function holdPoint() {
  return cam.pos.clone().add(new THREE.Vector3(0, -0.2, -2.6).multiplyScalar(BODY).applyQuaternion(cam.q))
}
function facing() {
  const f = new THREE.Vector3(0, 0, -1).applyQuaternion(cam.q)
  return Math.abs(f.x) > Math.abs(f.z) ? [Math.sign(f.x), 0, 0] : [0, 0, Math.sign(f.z) || -1]
}

function walkClick(e) {
  if (!walk.locked) { lockPointer() }
  const p = walk.center || centerPick()
  const sl = slots[walk.slot] || {}
  if (e.button === 2) {
    // the second use: a device turns a quarter, a brick goes
    if (p.sphere && S.W.spheres.get(p.sphere).cell) { const sp = S.W.spheres.get(p.sphere); edit({ op: 'rotate', ids: [sp.id], pivot: sp.cell, turns: 1 }); return }
    if (p.datum) { const d = S.W.datums.get(p.datum); edit({ op: 'rotate', ids: [d.id], pivot: d.cell, turns: 1 }); return }
    if (p.brick) edit({ op: 'remove', ids: [p.brick] })
    if (p.track) edit({ op: 'remove', ids: [p.track] })
    return
  }
  if (e.button === 1) { cycleSlot(1); return }
  if (e.button !== 0) return
  const k = sl.k || ''
  if (p.datum && (k === 'hand' || k === 'datum' && !e.shiftKey)) { toggleTile(p); return }
  if (k === 'datum') {
    if (p.cell && !p.sphere) edit({ op: 'datum', cell: p.cell, space: S.space })
    return
  }
  if (k === 'gimbal') {
    if (p.datum) { S.selDatum = p.datum; dirty(); status(datumText(datumView(p.datum)) + '  ·  wheel scales, Shift+wheel turns') }
    return
  }
  if (k === 'hand') {
    const sp = p.sphere && S.W.spheres.get(p.sphere)
    if (sp && sp.kind === 'src' && sp.params && 'on' in sp.params) toggleSwitch(sp)
    else if (sp) status(sp.kind + ' ' + sp.name + (sp.shown != null ? ' shows ' + sp.shown : ''))
    return
  }
  if (k === 'brick') {
    if (!walk.word) { askWord(); return }
    if (p.cell && !p.sphere) edit({ op: 'place', space: S.space, cell: p.cell, word: walk.word })
    return
  }
  if (k === 'wire') {
    if (!p.sphere) { status('aim at a device or sphere to wire', true); return }
    if (!S.wireFrom) { S.wireFrom = p.sphere; status('wire from ' + S.W.spheres.get(p.sphere).name + ': click where it goes'); return }
    const from = S.wireFrom
    S.wireFrom = null
    if (from !== p.sphere) edit({ op: 'link', from, to: p.sphere })
    return
  }
  if (k === 'inspect') {
    if (p.sphere) { S.selSphere = p.sphere; S.sel.clear(); S.selDatum = null } else if (p.datum) { S.selDatum = p.datum; S.selSphere = null; S.sel.clear() } else if (p.brick) { S.sel = new Set([p.brick]); S.selSphere = null; S.selDatum = null }
    dirty()
    walk.keepFree = true
    if (document.pointerLockElement) document.exitPointerLock()
    openInspector()
    return
  }
  if (k === 'remove') {
    if (p.sphere) edit({ op: 'remove', ids: [p.sphere] })
    else if (p.datum) edit({ op: 'remove', ids: [p.datum] })
    else if (p.brick) edit({ op: 'remove', ids: [p.brick] })
    else if (p.track) edit({ op: 'remove', ids: [p.track] })
    return
  }
  if (k === 'track') {
    if (p.cell && !p.sphere) trackClick(p.cell, true)
    return
  }
  if (k.startsWith('machine:')) {
    if (p.cell && !p.sphere && !p.track) placeMachine(k.slice(8), p.cell)
    return
  }
  if (k.startsWith('asm:')) {
    if (p.cell && !p.sphere) edit({ op: 'instance', assembly: k.slice(4), cell: p.cell, aim: facing(), space: S.space })
    return
  }
  if (SLOT_TEXT[k]) {
    if (p.cell && !p.sphere) edit({ op: 'sphere', kind: k, cell: p.cell, aim: facing(), space: S.space })
  }
}

// with the gimbal in hand, the wheel scales the cube aimed at by sixteenths, Shift+wheel turns it by 15°
function walkWheel(e) {
  const sl = slots[walk.slot] || {}
  const p = walk.center
  if (sl.k !== 'gimbal' || !p || !p.datum) return false
  const id = p.datum
  if (!walk.gim || walk.gim.id !== id) walk.gim = { id, parts: { ...datumView(id).parts } }
  const g = walk.gim
  const dir = e.deltaY < 0 ? 1 : -1
  const ax = walk.gaxis
  let set
  if (e.shiftKey) set = D.gimbalParts('r' + (ax || 'y'), g.parts, dir * D.ANGLE_STEP, { snap: true })
  else {
    const keys = ax ? ['s' + ax] : ['sx', 'sy', 'sz']
    set = {}
    for (const k of keys) set[k] = D.snapScale(Math.max(D.SCALE_STEP, Math.min(64, g.parts[k] + dir * 2 * D.SCALE_STEP)))
  }
  Object.assign(g.parts, set)
  S.datumPreview = { id, parts: { ...g.parts } }
  buildDatums()
  status(datumText(datumView(id)))
  clearTimeout(g.timer)
  g.timer = setTimeout(() => {
    const d = S.W.datums.get(id)
    walk.gim = null
    const base = d ? D.partsOf(d) : {}
    const changed = Object.fromEntries(Object.entries(g.parts).filter(([k, x]) => x !== base[k]))
    if (Object.keys(changed).length) edit({ op: 'transform', id, parts: changed }, true).then(() => { S.datumPreview = null; dirty() })
    else S.datumPreview = null
  }, 260)
  return true
}

function askWord() {
  walk.keepFree = true
  if (document.pointerLockElement) document.exitPointerLock()
  const inp = h('input', { value: walk.word, placeholder: 'a word, as typed on a brick' })
  inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') { $('#m-buttons .btn').click() } })
  modal('The brick in hand', [h('div', { class: 'note', text: 'Left click places this word on the face you aim at; the column grows as you stack.' }), inp], [['Hold it', () => { walk.word = inp.value.trim(); store.set('walk-word', walk.word); renderHotbar() }]])
  setTimeout(() => inp.focus(), 10)
}

function cycleSlot(d) {
  walk.slot = (walk.slot + d + slots.length) % slots.length
  S.wireFrom = null
  renderHotbar()
}
function renderHotbar() {
  const hb = $('#hotbar')
  hb.innerHTML = ''
  slots.forEach((sl, i) => {
    const label = sl.k === 'brick' ? (walk.word || 'brick') : SLOT_TEXT[sl.k] || sl.k.replace('asm:', '').replace('machine:', '')
    const col = SKCOL[sl.k] || (sl.k.startsWith('asm:') ? SKCOL.asm : sl.k.startsWith('machine:') ? MCOL[sl.k.slice(8)] : sl.k === 'track' ? '#3d3530' : null)
    const sw = SLOT_SWATCH[sl.k] || (col ? 'background:' + col : sl.k === 'brick' ? 'background:' + KCOL.lit : '')
    hb.append(h('div', { class: 'slot' + (i === walk.slot ? ' on' : ''), title: (i + 1) + ' · ' + (SLOT_TEXT[sl.k] || sl.k), onclick: () => { walk.slot = i; renderHotbar() } },
      h('span', { class: 'n', text: i < 9 ? String(i + 1) : '' }),
      h('span', { class: 'sw', style: sw }),
      h('span', { class: 'l', text: label })))
  })
}
function packDialog() {
  const all = [{ k: 'hand' }, { k: 'brick' }, { k: 'wire' }, ...['src', 'fn', 'ratchet', 'out', 'gate', 'mast', 'datum', 'gimbal', 'track'].map((k) => ({ k })), ...MACHINES.map((m) => ({ k: 'machine:' + m })), ...[...S.W.spaces.values()].filter((x) => x.owner === 'assembly').map((a) => ({ k: 'asm:' + a.name })), { k: 'inspect' }, { k: 'remove' }]
  const has = (k) => slots.some((x) => x.k === k)
  const body = all.map((it) => {
    const c = h('input', { type: 'checkbox', onchange: () => { if (c.checked && !has(it.k)) slots.push(it); if (!c.checked) slots = slots.filter((x) => x.k !== it.k); store.set('hotbar', slots); walk.slot = Math.min(walk.slot, slots.length - 1); renderHotbar() } })
    c.checked = has(it.k)
    return [c, h('span', { text: SLOT_TEXT[it.k] || it.k.replace('asm:', 'assembly ').replace('machine:', 'machine ') })]
  }).flat()
  modal('The bar', [h('div', { class: 'note', text: 'What the bar holds in walk mode. The wheel, the middle button and 1–9 pick from it.' }), h('div', { class: 'layers' }, ...body)], [['Reset', () => { slots = SLOT_DEFAULT.slice(); store.set('hotbar', slots); renderHotbar(); return true }]])
}

// E carries what the crosshair is on; E again sets it down where the crosshair is, Q drops it
function pickUp() {
  const p = walk.center || centerPick()
  if (p.datum) {
    walk.carryDatum = p.datum
    reindexDevices()
    status('carrying ' + S.W.datums.get(p.datum).name + ': E sets it down where you aim, Q drops it here')
    return
  }
  if (!p.sphere) { status('aim at a sphere, a device or a cube to carry it', true); return }
  const sp = S.W.spheres.get(p.sphere)
  if (sp.kind === 'gate') { status('a gate stays on its wall', true); return }
  walk.carry = sp.id
  reindexDevices()
  status('carrying ' + sp.name + ': E sets it down where you aim, Q drops it here')
}
function dropCarry(here) {
  if (walk.carryDatum) {
    const d = S.W.datums.get(walk.carryDatum)
    walk.carryDatum = null
    reindexDevices()
    if (!d) { dirty(); return }
    const p = walk.center || centerPick()
    const hp = holdPoint()
    const target = !here && p.cell && !p.sphere && !datumAt(S.space, p.cell) ? p.cell : [Math.floor(hp.x), Math.max(1, Math.floor(hp.y)), Math.floor(hp.z)]
    const delta = [target[0] - d.cell[0], target[1] - d.cell[1], target[2] - d.cell[2]]
    if (delta.some((x) => x)) edit({ op: 'move', ids: [d.id], delta })
    else dirty()
    return
  }
  if (!walk.carry) return
  const sp = S.W.spheres.get(walk.carry)
  walk.carry = null
  reindexDevices()
  if (!sp) { dirty(); return }
  const hp = holdPoint()
  if (sp.cell) {
    const p = walk.center || centerPick()
    const target = !here && p.cell && !p.sphere ? p.cell : [Math.floor(hp.x), sp.cell[1], Math.floor(hp.z)]
    const delta = [target[0] - sp.cell[0], target[1] - sp.cell[1], target[2] - sp.cell[2]]
    if (delta.some((x) => x)) edit({ op: 'move', ids: [sp.id], delta })
    else dirty()
  } else {
    const p = walk.center || centerPick()
    const at = !here && p.point ? [p.point.x, p.point.y + 1, p.point.z] : [hp.x, hp.y, hp.z]
    edit({ op: 'put', t: 'sphere', ...sp, pos: at.map((x) => Math.round(x * 4) / 4) })
  }
}

// R rides the link under the crosshair, then on along the links that follow
const linkRay = new THREE.Raycaster()
linkRay.params.Line.threshold = 0.25
function startRide() {
  // the link whose path passes nearest the crosshair ray, within reach
  ndc.set(0, 0)
  linkRay.setFromCamera(ndc, camera)
  let best = null, bd = 0.7 * 0.7
  for (const l of S.W.links.values()) {
    const a = S.W.spheres.get(l.from), b = S.W.spheres.get(l.to)
    if (!a || !b || !inSpace(a) || !inSpace(b)) continue
    const pts = ridePoints(l).map((p) => p.sub(new THREE.Vector3(0, 0.45, 0)))
    for (let i = 0; i + 1 < pts.length; i++) {
      const d = linkRay.ray.distanceSqToSegment(pts[i], pts[i + 1])
      if (d < bd) { bd = d; best = l }
    }
  }
  if (!best) { status('aim at a wire, a beam or a link to ride it', true); return }
  const l = best
  walk.ride = { link: l, seen: new Set([l.id]), pts: ridePoints(l), d: 0, n: 0 }
  status('riding from ' + S.W.spheres.get(l.from).name + ' — R or Esc gets off')
}
function ridePoints(l) {
  const a = sphereView(l.from), b = sphereView(l.to)
  const wire = l.mode !== 'ray' && (a.cell || b.cell)
  const pts = wire ? wirePath(a, b, l.route || []) : [new THREE.Vector3(...a.pos), new THREE.Vector3(...b.pos)]
  return pts.map((p) => p.clone().add(new THREE.Vector3(0, 0.45, 0)))
}
function rideFrame(dt) {
  const rd = walk.ride
  rd.d += dt * 7
  let d = rd.d
  for (let i = 0; i + 1 < rd.pts.length; i++) {
    const seg = rd.pts[i].distanceTo(rd.pts[i + 1])
    if (d <= seg) {
      const p = rd.pts[i].clone().lerp(rd.pts[i + 1], seg ? d / seg : 1)
      const dir = rd.pts[i + 1].clone().sub(rd.pts[i])
      cam.pos.copy(p)
      if (dir.lengthSq() > 1e-6) {
        const yaw = Math.atan2(-dir.x, -dir.z)
        walk.yaw += Math.atan2(Math.sin(yaw - walk.yaw), Math.cos(yaw - walk.yaw)) * Math.min(1, dt * 6)
        walk.pitch += (Math.atan2(dir.y, Math.hypot(dir.x, dir.z)) * 0.6 - walk.pitch) * Math.min(1, dt * 4)
      }
      cam.q.setFromEuler(new THREE.Euler(walk.pitch, walk.yaw, 0, 'YXZ'))
      pulseAt(rd.link.id)
      return
    }
    d -= seg
  }
  // at the end of a link: on along the next one, as a pulse would go
  const next = [...S.W.links.values()].filter((x) => x.from === rd.link.to && !rd.seen.has(x.id)).sort((a, b) => a.id - b.id)[0]
  if (next && rd.n < 24) {
    rd.seen.add(next.id)
    rd.link = next
    rd.pts = ridePoints(next)
    rd.d = 0
    rd.n++
    status('riding on to ' + S.W.spheres.get(next.to).name)
    return
  }
  const end = rd.pts[rd.pts.length - 1]
  walk.ride = null
  walk.p = newPlayer([end.x, end.y, end.z])
  while (playerHits(walk.p.feet, WALK.h, solidAt)) walk.p.feet[1] += 1
  status('off the ride at ' + S.W.spheres.get(rd.link.to).name)
}
let lastPulse = 0
function pulseAt(lid) {
  const t = performance.now()
  if (t - lastPulse > 300) { lastPulse = t; pulse(lid) }
}

function walkKey(e) {
  const k = e.key
  if (walk.ride) {
    if (k === 'r' || k === 'R' || k === 'Escape') { walk.ride = null; walk.p = newPlayer([cam.pos.x, cam.pos.y - WALK.eye, cam.pos.z]); while (playerHits(walk.p.feet, WALK.h, solidAt)) walk.p.feet[1] += 1; status('off the ride') }
    return true
  }
  if (k === 'f' || k === 'F') { setWalk(false); return true }
  if (k === 'e' || k === 'E') { walk.carry ? dropCarry(false) : pickUp(); return true }
  if (k === 'q' || k === 'Q') { dropCarry(true); return true }
  if (k === 'r' || k === 'R') { startRide(); return true }
  if (k === 'Enter') { askWord(); return true }
  if (/^[xyzXYZ]$/.test(k) && (slots[walk.slot] || {}).k === 'gimbal') { const a = k.toLowerCase(); walk.gaxis = walk.gaxis === a ? '' : a; return true }
  if (k === 'Escape') { walk.keepFree = false; if (document.pointerLockElement) document.exitPointerLock(); else showMenu(); return true }
  if (/^[1-9]$/.test(k)) { const i = +k - 1; if (i < slots.length) { walk.slot = i; S.wireFrom = null; renderHotbar() } return true }
  if (['KeyW', 'KeyA', 'KeyS', 'KeyD', 'Space', 'ShiftLeft', 'ShiftRight', 'ControlLeft', 'ControlRight', 'KeyC'].includes(e.code)) {
    e.preventDefault()
    walk.keys.add(e.code)
    return true
  }
  return false
}

// ---------- tutorial ----------

// chapters are the level worlds in order; each goal may carry guide steps, each waiting
// for one event: an edit op (place, sphere, link, setdown, assemble, …), run, or solved
const tut = { on: store.get('tutorial', false), steps: [], i: 0 }
async function showLevels() {
  const r = await api('list')
  const done = store.get('stars', {})
  const levels = (r.levels || []).sort((a, b) => (a.order ?? 99) - (b.order ?? 99))
  const body = levels.map((l) => h('div', { class: 'level', onclick: async () => { $('#modal').hidden = true; await startChapter(l.name) } },
    h('span', { class: 'stars', text: '★'.repeat(done[l.name] || 0) + '☆'.repeat(3 - (done[l.name] || 0)) }), h('b', { text: l.title || l.name })))
  modal('Tutorial', [h('div', { class: 'note', text: 'Chapters open as worlds of their own, so what you build is kept. The guide follows each step.' }), ...(body.length ? body : [h('div', { class: 'note', text: 'No levels folder found.' })])])
}
async function startChapter(name) {
  tut.on = true
  store.set('tutorial', true)
  await openWorld('level-' + name, name)
}
function guideOpen() {
  const g = S.W.goal
  const el = $('#guide')
  if (!tut.on || !g || !S.name.startsWith('level-')) { el.hidden = true; return }
  tut.steps = (g.steps && g.steps.length ? g.steps : [{ on: 'solved', text: g.text }])
  tut.i = Math.min(store.get('tut-' + S.name, 0), tut.steps.length)
  el.hidden = false
  renderGuide()
}
function renderGuide() {
  const g = S.W.goal
  if (!g) return
  $('#g-title').textContent = g.title || 'tutorial'
  const b = $('#g-body')
  b.innerHTML = ''
  const done = tut.i >= tut.steps.length
  if (g.text && tut.steps.length > 1) b.append(h('div', { class: 'note', text: g.text }))
  b.append(h('div', { class: 'steps', text: tut.steps.map((x, i) => (i < tut.i ? '■' : i === tut.i ? '▣' : '□')).join(' ') }))
  b.append(h('div', { class: 'step', text: done ? 'Every step is done. Check the goal for stars, or go on.' : tut.steps[tut.i].text }))
  b.append(h('div', { class: 'row' },
    h('button', { class: 'btn', text: 'Check', onclick: checkGoal }),
    h('button', { class: 'btn', text: 'Next chapter', onclick: nextChapter }),
    h('button', { class: 'btn', text: 'Chapters', onclick: showLevels })))
}
function tutorialEvent(kind) {
  if (!tut.on || $('#guide').hidden || tut.i >= tut.steps.length) return
  if (tut.steps[tut.i].on === kind) {
    tut.i++
    store.set('tut-' + S.name, tut.i)
    renderGuide()
  }
}
async function nextChapter() {
  const r = await api('list')
  const levels = (r.levels || []).sort((a, b) => (a.order ?? 99) - (b.order ?? 99))
  const cur = S.name.replace(/^level-/, '')
  const i = levels.findIndex((l) => l.name === cur)
  const next = levels[i + 1]
  if (next) await startChapter(next.name)
  else status('that was the last chapter')
}
$('#g-close').onclick = () => { tut.on = false; store.set('tutorial', false); $('#guide').hidden = true }

// ---------- themes ----------

const THEMES = {
  milk: {
    win: '#fbf6ee', sky: 0xe9dbc6, ink: '#3b2d20', ink2: '#6d5a47', accent: '#7a5532', err: '#b0413e', face: '#2e2216',
    grid: [0xb79c78, 0xd2bd9c], plateLine: 0xb79c78, ghost: 0x7a5532, hover: 0x3b2d20,
    plate: { main: '#d6c09d', script: '#c5d4b2', shelf: '#d5bfd4', inside: '#cbc2ae' },
    brick: null, sphere: null,
  },
  monokai: {
    win: '#241d24', sky: 0x000000, ink: '#f6eef3', ink2: '#a898a4', accent: '#ff5fa2', err: '#ff6b6b', face: '#120e12',
    grid: [0x4a3c4a, 0x2b232b], plateLine: 0x4a3c4a, ghost: 0xff5fa2, hover: 0xf6eef3,
    plate: { main: '#2b232b', script: '#22302a', shelf: '#302232', inside: '#262026' },
    brick: { lit: '#f6eef3', num: '#ae81ff', esc: '#a898a4', name: '#66d9ef', read: '#8fe3f3', box: '#d9f0b0', quote: '#e6db74', lib: '#fd971f', geo: '#ff9ac7', sock: '#c3ef7a', raw: '#d8ccd4',
      io: '#ff5fa2', file: '#ff7ab4', cond: '#f78fb3', list: '#c9a8ff', code: '#ff5fa2', kern: '#66d9ef', thread: '#a6e22e', numc: '#ae81ff' },
    sphere: { src: '#fd971f', fn: '#66d9ef', ratchet: '#ff5fa2', out: '#a6e22e', gate: '#a898a4', mast: '#e6db74', port: '#f78fb3', asm: '#ae81ff' },
  },
  zen: {
    win: '#ffffff', sky: 0xffffff, ink: '#000000', ink2: '#5a5a5a', accent: '#000000', err: '#000000', face: '#ffffff',
    grid: [0x000000, 0xd0d0d0], plateLine: 0x9a9a9a, ghost: 0x000000, hover: 0x000000,
    plate: { main: '#f2f2f2', script: '#ebebeb', shelf: '#e4e4e4', inside: '#f2f2f2' },
    brick: 'mono', sphere: { src: '#000000', fn: '#2a2a2a', ratchet: '#444444', out: '#5a5a5a', gate: '#7a7a7a', mast: '#1a1a1a', port: '#333333', asm: '#111111' },
  },
}
const BASE = { brick: { ...KCOL }, sphere: { ...SKCOL }, plate: { ...PCOL } }
let theme = store.get('theme', 'milk')
function TH() { return THEMES[theme] || THEMES.milk }
function applyTheme(name) {
  theme = THEMES[name] ? name : 'milk'
  store.set('theme', theme)
  document.documentElement.dataset.theme = theme
  const t = TH()
  scene.background = new THREE.Color(t.sky)
  scene.fog.color = new THREE.Color(t.sky)
  Object.assign(PCOL, t.plate)
  if (t.brick === 'mono') for (const k in KCOL) KCOL[k] = '#1c1c1c'
  else Object.assign(KCOL, BASE.brick, t.brick || {})
  Object.assign(SKCOL, BASE.sphere, t.sphere || {})
  faceMat.uniforms && faceMat.uniforms.uColor && faceMat.uniforms.uColor.value.set(t.face)
  ghost.material.color.set(t.ghost)
  hoverRing.material.color.set(t.hover)
  if (gridMesh) { gridMesh.geometry.dispose(); gridMesh = null }
  dirty()
}

// ---------- the menu ----------

function showMenu() {
  const m = $('#menu')
  if (!m.hidden) { m.hidden = true; return }
  m.innerHTML = ''
  const item = (text, f, on = false) => m.append(h('button', { class: on ? 'on' : '', text, onclick: () => { m.hidden = true; f() } }))
  const sub = (text) => m.append(h('div', { class: 'sub', text }))
  sub('world')
  item('New world…', newWorldDialog)
  item('Import, export…', showFile)
  item('Tutorial…', showLevels)
  sub('theme')
  for (const [k, n] of [['milk', 'coffee with milk'], ['monokai', 'monokai pink'], ['zen', 'zen white and black']]) item(n, () => applyTheme(k), theme === k)
  sub('view')
  item(walk.on ? 'Fly (F)' : 'Walk (F)', () => setWalk(!walk.on))
  item('Pack the bar…', packDialog)
  item('Layers…', showView)
  item('Reset the window layout', resetLayout)
  item('Keys…', showKeys)
  const r = $('#b-menu').getBoundingClientRect()
  m.style.left = r.left + 'px'
  if (r.top > innerHeight / 2) { m.style.top = 'auto'; m.style.bottom = (innerHeight - r.top + 4) + 'px' } else { m.style.bottom = 'auto'; m.style.top = (r.bottom + 4) + 'px' }
  m.hidden = false
}
$('#b-menu').onclick = (e) => { e.stopPropagation(); showMenu() }
addEventListener('mousedown', (e) => { if (!$('#menu').hidden && !e.target.closest('#menu, #b-menu')) $('#menu').hidden = true })

// ---------- the console ----------

const conIn = $('#con-in')
const conHist = store.get('con-hist', [])
let conAt = conHist.length
conIn.addEventListener('keydown', async (e) => {
  e.stopPropagation()
  if (e.key === '`') { e.preventDefault(); dropConsole(); canvas.focus(); return }
  if (e.key === 'ArrowUp') { e.preventDefault(); conAt = Math.max(0, conAt - 1); conIn.value = conHist[conAt] || ''; return }
  if (e.key === 'ArrowDown') { e.preventDefault(); conAt = Math.min(conHist.length, conAt + 1); conIn.value = conHist[conAt] || ''; return }
  if (e.key !== 'Enter') return
  const t = conIn.value
  if (!t.trim()) return
  conHist.push(t)
  while (conHist.length > 200) conHist.shift()
  store.set('con-hist', conHist)
  conAt = conHist.length
  conIn.value = ''
  log('› ' + t, 'tr')
  const r = await api('eval', { text: t })
  if (r.error) { log(r.error, 'err'); return }
  if (S.run && r.zones) showReport(r)
  else if (r.out) $('#out').append(document.createTextNode(r.out))
  if (r.status && r.status !== 'ok') log(r.status, 'err')
  if (r.line && r.line.length) log('line: ' + r.line.join(' '), 'tr')
  $('#out').scrollTop = 1e9
})

// ---------- frame loop ----------

function resize() {
  const w = innerWidth, hh = innerHeight
  renderer.setSize(w, hh, false)
  camera.aspect = w / hh
  camera.updateProjectionMatrix()
  T.shared.uViewH.value = hh * renderer.getPixelRatio()
}
addEventListener('resize', resize)
let last = performance.now()
function frameLoop(now) {
  const dt = Math.min(0.05, (now - last) / 1000)
  last = now
  if (walk.on) walkFrame(dt)
  else moveCamera(dt)
  camera.position.copy(cam.pos)
  camera.quaternion.copy(cam.q)
  if (needBuild) build()
  else if (S.W.datums.size && datums.stale(cam.pos.toArray())) buildDatums()
  updateDots(dt)
  updateItems(now)
  placeEditor()
  if (S.run && S.lastMain) showStack(S.lastMain)
  if (now - miniAt > 120) { miniAt = now; drawMinimap() }
  renderer.render(scene, camera)
  requestAnimationFrame(frameLoop)
}

async function boot() {
  initWindows()
  applyTheme(theme)
  resize()
  status('loading fonts…')
  await T.loadFonts(['fonts/ibm-plex-mono-latin-400-normal.woff', 'fonts/ibm-plex-mono-latin-ext-400-normal.woff'])
  await loadPalette()
  await openWorld(S.name)
  requestAnimationFrame(frameLoop)
  if (!store.get('seen-keys', false)) { store.set('seen-keys', true); showKeys() }
}
boot().catch((e) => { status(String(e), true); console.error(e) })
window.__s3 = S
window.__s3scene = scene
window.__s3walk = () => walk
window.__s3edit = edit
window.__s3open = openWorld
window.__s3screen = (x, y, z) => toScreen(new THREE.Vector3(x, y, z))
window.__s3datums = () => ({ ...datumLod, gimbal: gizmo.where() && { c: gizmo.where().c.toArray(), ax: gizmo.where().ax.map((a) => a.toArray()), ext: gizmo.where().ext } })
window.__s3cam = (p, t) => { cam.pos.set(...p); lookAt(new THREE.Vector3(...t)); camera.position.copy(cam.pos); camera.quaternion.copy(cam.q); camera.updateMatrixWorld() }
window.__s3pick = (x, y) => { const p = pick(x, y); return JSON.stringify({ ...p, point: p.point && p.point.toArray() }) }
