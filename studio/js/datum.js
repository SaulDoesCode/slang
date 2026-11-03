// Datum cubes without a scene: tiles and bits, the cube's frame, ray hits, level of detail
// and the gimbal's arithmetic. main.js draws with these; test/datum.test.mjs checks them.

export const FACE_NAMES = ['+x', '-x', '+y', '-y', '+z', '-z']

// each face: its normal, and its u and v axes as seen from outside (u to the right, v down)
export const FACES = [
  { n: [1, 0, 0], u: [0, 0, -1], v: [0, -1, 0] },
  { n: [-1, 0, 0], u: [0, 0, 1], v: [0, -1, 0] },
  { n: [0, 1, 0], u: [1, 0, 0], v: [0, 0, 1] },
  { n: [0, -1, 0], u: [-1, 0, 0], v: [0, 0, 1] },
  { n: [0, 0, 1], u: [1, 0, 0], v: [0, -1, 0] },
  { n: [0, 0, -1], u: [-1, 0, 0], v: [0, -1, 0] },
]

// the cube in its own frame: x and z from −½ to ½, y from 0 to 1, so it stands on its cell's floor
export const CENTER = [0, 0.5, 0]

export const NEAR = 12
export const PART_NAMES = ['sx', 'sy', 'sz', 'rx', 'ry', 'rz']
export const PART_DEFAULT = { sx: 1, sy: 1, sz: 1, rx: 0, ry: 0, rz: 0 }

// the bit a tile carries: ±x red, ±y green, ±z blue; + the high nibble; tile (0, 0) its highest bit
export function tileBit(face, u, v) {
  const channel = Math.floor(face / 2)
  const base = (2 - channel) * 8 + (face % 2 === 0 ? 4 : 0)
  return base + 3 - (v * 2 + u)
}

export function tileOfBit(bit) {
  for (let f = 0; f < 6; f++) for (let v = 0; v < 2; v++) for (let u = 0; u < 2; u++) if (tileBit(f, u, v) === bit) return { face: f, u, v }
  return null
}

// a tile's center in the cube's frame, with its face's axes
export function tileCenter(face, u, v) {
  const F = FACES[face]
  const c = [0, 1, 2].map((i) => CENTER[i] + F.n[i] * 0.5 + F.u[i] * (u - 0.5) * 0.5 + F.v[i] * (v - 0.5) * 0.5)
  return { c, n: F.n, u: F.u, v: F.v }
}

export function colorOf(value) {
  return '#' + ((value >>> 0) & 0xffffff).toString(16).padStart(6, '0')
}

export function rgbOf(value) {
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255]
}

// the transform parts of a record or of what code left, all six with defaults filled in
export function partsOf(d, rt) {
  const p = { ...PART_DEFAULT }
  if (d.scale) [p.sx, p.sy, p.sz] = d.scale
  if (d.rot) [p.rx, p.ry, p.rz] = d.rot
  if (rt && rt.parts) for (const k of PART_NAMES) p[k] = k in rt.parts ? rt.parts[k] : PART_DEFAULT[k]
  return p
}

// the parts that differ from standard: the schema
export function schemaOf(parts) {
  return PART_NAMES.filter((k) => parts[k] !== PART_DEFAULT[k])
}

const rad = (d) => (d * Math.PI) / 180

// rotation in degrees about x, y, z, applied z first, then y, then x: R = Rx Ry Rz (row-major 3×3)
export function rotation(rx, ry, rz) {
  const [a, b, c] = [rad(rx), rad(ry), rad(rz)]
  const ca = Math.cos(a), sa = Math.sin(a), cb = Math.cos(b), sb = Math.sin(b), cc = Math.cos(c), sc = Math.sin(c)
  return [
    [cb * cc, -cb * sc, sb],
    [ca * sc + sa * sb * cc, ca * cc - sa * sb * sc, -sa * cb],
    [sa * sc - ca * sb * cc, sa * cc + ca * sb * sc, ca * cb],
  ]
}

const mul = (R, v) => [0, 1, 2].map((i) => R[i][0] * v[0] + R[i][1] * v[1] + R[i][2] * v[2])
const mulT = (R, v) => [0, 1, 2].map((i) => R[0][i] * v[0] + R[1][i] * v[1] + R[2][i] * v[2])

// where the cube's frame sits in the world: the center of its cell's floor
export function pivotOf(cell) {
  return [cell[0] + 0.5, cell[1], cell[2] + 0.5]
}

// a point of the cube's frame in the world, and back
export function toWorld(cell, parts, p) {
  const R = rotation(parts.rx, parts.ry, parts.rz)
  const s = [p[0] * parts.sx, p[1] * parts.sy, p[2] * parts.sz]
  const r = mul(R, s)
  const o = pivotOf(cell)
  return [o[0] + r[0], o[1] + r[1], o[2] + r[2]]
}
export function toLocal(cell, parts, p) {
  const R = rotation(parts.rx, parts.ry, parts.rz)
  const o = pivotOf(cell)
  const r = mulT(R, [p[0] - o[0], p[1] - o[1], p[2] - o[2]])
  return [r[0] / parts.sx, r[1] / parts.sy, r[2] / parts.sz]
}
export function dirToWorld(parts, d) {
  return mul(rotation(parts.rx, parts.ry, parts.rz), d)
}
function dirToLocal(parts, d) {
  const r = mulT(rotation(parts.rx, parts.ry, parts.rz), d)
  return [r[0] / parts.sx, r[1] / parts.sy, r[2] / parts.sz]
}

// the 16 numbers of the cube's matrix, column-major as THREE.Matrix4.fromArray takes them
export function matrixOf(cell, parts) {
  const R = rotation(parts.rx, parts.ry, parts.rz)
  const s = [parts.sx, parts.sy, parts.sz]
  const o = pivotOf(cell)
  const m = new Array(16).fill(0)
  for (let c = 0; c < 3; c++) for (let r = 0; r < 3; r++) m[c * 4 + r] = R[r][c] * s[c]
  m[12] = o[0]; m[13] = o[1]; m[14] = o[2]; m[15] = 1
  return m
}

// which face, and which tile on it, a point on the cube's surface (in its frame) is on
export function tileAt(local, normal) {
  let face = 0, best = -1
  for (let f = 0; f < 6; f++) {
    const n = FACES[f].n
    const d = n[0] * normal[0] + n[1] * normal[1] + n[2] * normal[2]
    if (d > best) { best = d; face = f }
  }
  const F = FACES[face]
  const q = [local[0] - CENTER[0], local[1] - CENTER[1], local[2] - CENTER[2]]
  const du = q[0] * F.u[0] + q[1] * F.u[1] + q[2] * F.u[2]
  const dv = q[0] * F.v[0] + q[1] * F.v[1] + q[2] * F.v[2]
  return { face, u: du < 0 ? 0 : 1, v: dv < 0 ? 0 : 1 }
}

// a ray (world) against a datum: the distance along the ray, the point, and the tile it lands on
export function rayHit(cell, parts, origin, dir) {
  const o = toLocal(cell, parts, origin)
  const d = dirToLocal(parts, dir)
  const lo = [-0.5, 0, -0.5], hi = [0.5, 1, 0.5]
  let t0 = -Infinity, t1 = Infinity, axis = -1, sign = 0
  for (let i = 0; i < 3; i++) {
    if (Math.abs(d[i]) < 1e-12) {
      if (o[i] < lo[i] || o[i] > hi[i]) return null
      continue
    }
    let a = (lo[i] - o[i]) / d[i], b = (hi[i] - o[i]) / d[i]
    let s = -1
    if (a > b) { [a, b] = [b, a]; s = 1 }
    if (a > t0) { t0 = a; axis = i; sign = s }
    if (b < t1) t1 = b
    if (t0 > t1) return null
  }
  // from inside the cube there is no face to click
  if (t0 < 0 || axis < 0) return null
  const t = t0
  const local = [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t]
  const normal = [0, 0, 0]
  normal[axis] = sign
  // t is in the ray's own units: the world distance is t × |dir| since dir was not rescaled
  const len = Math.hypot(dir[0], dir[1], dir[2]) || 1
  return { t: t * len, local, normal, ...tileAt(local, normal), point: toWorld(cell, parts, local) }
}

// near within NEAR cells of the eye: tiles; beyond: one flat color
export function lod(cell, parts, eye) {
  const c = toWorld(cell, parts, CENTER)
  return Math.hypot(c[0] - eye[0], c[1] - eye[1], c[2] - eye[2]) <= NEAR ? 'near' : 'far'
}

// which face of the cube faces a world direction: rotating a cube moves its nibbles with its faces
export function faceToward(parts, worldDir) {
  const d = mulT(rotation(parts.rx, parts.ry, parts.rz), worldDir)
  return tileAt(CENTER, d).face
}

// ---------- the gimbal ----------

export const SCALE_STEP = 1 / 16
export const ANGLE_STEP = 15

export const snapScale = (x) => Math.max(SCALE_STEP, Math.round(x / SCALE_STEP) * SCALE_STEP)
export const snapAngle = (a) => Math.round(a / ANGLE_STEP) * ANGLE_STEP

export function normDeg(x) {
  let r = x % 360
  if (r > 180) r -= 360
  if (r <= -180) r += 360
  return r === 0 ? 0 : r
}

// the axes a handle changes: an axis, a plane of two, or all three
export const HANDLES = {
  sx: ['sx'], sy: ['sy'], sz: ['sz'],
  sxy: ['sx', 'sy'], syz: ['sy', 'sz'], sxz: ['sx', 'sz'],
  s: ['sx', 'sy', 'sz'],
  rx: ['rx'], ry: ['ry'], rz: ['rz'],
}

// X, Y or Z held while dragging constrains a scale handle to that axis, a ring to that axis
export function constrain(handle, key) {
  const a = { x: 'x', y: 'y', z: 'z' }[(key || '').toLowerCase()]
  if (!a) return handle
  return handle.startsWith('r') ? 'r' + a : 's' + a
}

// The parts a drag sets. Scale: amount is a factor (1 = unchanged); rotation: amount in degrees
// added. Ctrl snaps; a typed number sets the amount exactly (a factor, or degrees).
export function gimbalParts(handle, start, amount, opts = {}) {
  const keys = HANDLES[handle] || []
  const out = {}
  const typed = opts.typed != null && opts.typed !== '' && isFinite(+opts.typed) ? +opts.typed : null
  for (const k of keys) {
    if (k[0] === 's') {
      let x = typed != null ? typed : start[k] * amount
      if (opts.snap) x = snapScale(x)
      out[k] = Math.min(64, Math.max(SCALE_STEP, x))
    } else {
      let x = typed != null ? typed : start[k] + amount
      if (opts.snap) x = snapAngle(x)
      out[k] = normDeg(x)
    }
  }
  return out
}

// scale factor from a drag along a handle's screen axis: px is the drag projected on that axis,
// unit the screen length of one cell along it
export function dragFactor(px, unit, size) {
  return Math.max(SCALE_STEP / Math.max(size, SCALE_STEP), (size + px / Math.max(unit, 1e-6)) / Math.max(size, 1e-6))
}

// angle swept around a screen point between two pointer positions, in degrees
export function sweep(center, a, b) {
  const t0 = Math.atan2(a[1] - center[1], a[0] - center[0])
  const t1 = Math.atan2(b[1] - center[1], b[0] - center[0])
  let d = ((t1 - t0) * 180) / Math.PI
  while (d > 180) d -= 360
  while (d < -180) d += 360
  return d
}

// the log a datum would show after the parts are set, as the server writes it
export function partLines(name, parts) {
  return Object.entries(parts).map(([k, x]) => numText(x) + ' ' + name + '/' + k + '#')
}
export const numText = (x) => String(x)
