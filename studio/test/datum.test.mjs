// node studio/test/datum.test.mjs
import * as D from '../js/datum.js'
import assert from 'node:assert/strict'

const near = (a, b, e = 1e-9) => Math.abs(a - b) < e
const nearV = (a, b, e = 1e-9) => a.every((x, i) => near(x, b[i], e))
const std = { ...D.PART_DEFAULT }

// 24 tiles, 24 bits, the same order as the server's tile_bit
const bits = new Set()
for (let f = 0; f < 6; f++) for (let v = 0; v < 2; v++) for (let u = 0; u < 2; u++) bits.add(D.tileBit(f, u, v))
assert.equal(bits.size, 24)
assert.equal(D.tileBit(0, 0, 0), 23)
assert.equal(D.tileBit(1, 1, 1), 16)
assert.equal(D.tileBit(2, 0, 0), 15)
assert.equal(D.tileBit(5, 1, 1), 0)
for (let b = 0; b < 24; b++) { const t = D.tileOfBit(b); assert.equal(D.tileBit(t.face, t.u, t.v), b) }

// colors
assert.equal(D.colorOf(0xffa500), '#ffa500')
assert.deepEqual(D.rgbOf(0xffa500), [255, 165, 0])

// each tile's center is on its face, and clicking it finds that tile again
for (let f = 0; f < 6; f++) for (let v = 0; v < 2; v++) for (let u = 0; u < 2; u++) {
  const t = D.tileCenter(f, u, v)
  assert.deepEqual(D.tileAt(t.c, t.n), { face: f, u, v })
}

// a ray straight at the +x face of a standard cube at cell 3 1 4 hits tile (0, 0): top, toward +z, the left as seen from +x
let h = D.rayHit([3, 1, 4], std, [10, 1.75, 4.75], [-1, 0, 0])
assert.ok(h, 'hits')
assert.ok(near(h.t, 6), 'distance ' + h.t)
assert.deepEqual([h.face, h.u, h.v], [0, 0, 0])
assert.equal(D.tileBit(h.face, h.u, h.v), 23)
// the same tile seen after a quarter turn about y: the +x face now looks toward -z
const turned = { ...std, ry: 90 }
assert.equal(D.faceToward(turned, [0, 0, -1]), 0, 'the red high nibble turned to face -z')
h = D.rayHit([3, 1, 4], turned, [3.5 - 0.25, 1.75, -5], [0, 0, 1])
assert.ok(h, 'hits the turned cube')
assert.equal(h.face, 0)
// a scaled cube is hit at its stretched face and reports the right tile
const wide = { ...std, sx: 4 }
h = D.rayHit([0, 1, 0], wide, [10, 1.25, 0.75], [-1, 0, 0])
assert.ok(near(h.t, 10 - 2.5), 'wide face at x = 2.5: ' + h.t)
assert.deepEqual([h.face, h.u, h.v], [0, 0, 1])
// misses and inside
assert.equal(D.rayHit([0, 1, 0], std, [10, 5, 0.5], [-1, 0, 0]), null)
assert.equal(D.rayHit([0, 1, 0], std, [0.5, 1.5, 0.5], [-1, 0, 0]), null)

// frame: toWorld and toLocal are inverse, and the matrix agrees with toWorld
const p = { sx: 2.5, sy: 0.5, sz: 3, rx: 30, ry: -45, rz: 15 }
const w = D.toWorld([2, 3, -1], p, [0.3, 0.7, -0.2])
assert.ok(nearV(D.toLocal([2, 3, -1], p, w), [0.3, 0.7, -0.2], 1e-9))
const m = D.matrixOf([2, 3, -1], p)
const l = [0.3, 0.7, -0.2]
const mw = [0, 1, 2].map((r) => m[r] * l[0] + m[4 + r] * l[1] + m[8 + r] * l[2] + m[12 + r])
assert.ok(nearV(mw, w, 1e-9))

// level of detail at 12 cells from the cube's center
assert.equal(D.lod([0, 1, 0], std, [0.5, 1.5, 12.4]), 'near')
assert.equal(D.lod([0, 1, 0], std, [0.5, 1.5, 12.6]), 'far')
assert.equal(D.lod([0, 1, 0], { ...std, sy: 30 }, [0.5, 16, 10]), 'near', 'a tall cube is measured from its center')

// schema: only what differs from standard
assert.deepEqual(D.schemaOf(D.partsOf({ scale: [2, 1, 1], rot: [0, 45, 0] })), ['sx', 'ry'])
assert.deepEqual(D.schemaOf(D.partsOf({}, { parts: { sz: 3 } })), ['sz'])

// gimbal: axis handle, plane handle, uniform, snapping, typed amounts, constraint keys
const start = { sx: 1, sy: 2, sz: 1, rx: 0, ry: 170, rz: 0 }
assert.deepEqual(D.gimbalParts('sx', start, 1.5), { sx: 1.5 })
assert.deepEqual(D.gimbalParts('syz', start, 2), { sy: 4, sz: 2 })
assert.deepEqual(D.gimbalParts('s', start, 0.5), { sx: 0.5, sy: 1, sz: 0.5 })
assert.deepEqual(D.gimbalParts('sx', start, 1.04, { snap: true }), { sx: 1.0625 })
assert.deepEqual(D.gimbalParts('sx', start, 9, { typed: '2.25' }), { sx: 2.25 })
assert.deepEqual(D.gimbalParts('sx', start, 0.0001), { sx: 1 / 16 }, 'scale stays above zero')
assert.deepEqual(D.gimbalParts('ry', start, 20), { ry: -170 }, 'angles wrap')
assert.deepEqual(D.gimbalParts('ry', start, 7, { snap: true }), { ry: 180 })
assert.deepEqual(D.gimbalParts('rx', start, 3, { typed: '-90' }), { rx: -90 })
assert.equal(D.constrain('s', 'Y'), 'sy')
assert.equal(D.constrain('rx', 'z'), 'rz')
assert.equal(D.constrain('sxy', 'q'), 'sxy')
assert.ok(near(D.dragFactor(50, 100, 1), 1.5))
assert.ok(near(D.sweep([0, 0], [1, 0], [0, 1]), 90))
assert.ok(near(D.sweep([0, 0], [-1, 0.001], [-1, -0.001]), 0.1146, 1e-3))
assert.deepEqual(D.partLines('c7', { sx: 2.5, ry: 45 }), ['2.5 c7/sx#', '45 c7/ry#'])

console.log('datum: ok')
