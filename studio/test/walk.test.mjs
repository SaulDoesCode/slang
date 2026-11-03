// node studio/test/walk.test.mjs
import { newPlayer, stepPlayer, hits, P } from '../js/walk.js'
import assert from 'node:assert/strict'

const blocks = new Set()
const key = (x, y, z) => x + ',' + y + ',' + z
const solid = (x, y, z) => y <= 0 || blocks.has(key(x, y, z))
const run = (p, inp, secs) => { for (let t = 0; t < secs; t += 1 / 120) stepPlayer(p, { fwd: 0, right: 0, yaw: 0, ...inp }, 1 / 120, solid); return p }

// falls to the ground and stands on it
let p = run(newPlayer([0.5, 5, 0.5]), {}, 2)
assert.ok(Math.abs(p.feet[1] - 1) < 1e-9 && p.ground, 'lands on the ground: ' + p.feet)

// walks forward (-z at yaw 0) at walking speed
p = run(newPlayer([0.5, 1, 0.5]), { fwd: 1 }, 1)
assert.ok(p.feet[2] < -3.5 && Math.abs(p.feet[0] - 0.5) < 1e-6, 'walks: ' + p.feet)
assert.ok(Math.abs(Math.hypot(p.vel[0], p.vel[2]) - P.walk) < 0.01)

// a half-height step is not a thing here (cells are whole), but a one-cell ledge is climbed only by a vault
blocks.add(key(0, 1, -3))
p = run(newPlayer([0.5, 1, 0.5]), { fwd: 1 }, 1)
assert.ok(p.feet[2] > -2.35 && p.feet[1] === 1, 'a wall one cell high stops a walk: ' + p.feet)
p = run(newPlayer([0.5, 1, 0.5]), { fwd: 1, jump: true }, 1)
assert.ok(p.feet[2] < -3 && p.feet[1] >= 1, 'a jump vaults it: ' + p.feet)

// two cells high cannot be vaulted
blocks.add(key(0, 2, -3))
p = run(newPlayer([0.5, 1, 0.5]), { fwd: 1, jump: true }, 1.5)
assert.ok(p.feet[2] > -2.35, 'two cells stop a vault: ' + p.feet)

// crouching under a one-cell gap
blocks.clear()
for (let z = -6; z <= -3; z++) blocks.add(key(0, 2, z))
p = run(newPlayer([0.5, 1, 0.5]), { fwd: 1 }, 1)
assert.ok(p.feet[2] > -2.4, 'standing does not fit under: ' + p.feet)
p = run(newPlayer([0.5, 1, 0.5]), { fwd: 1, crouch: true }, 4)
assert.ok(p.feet[2] < -6, 'crouching does: ' + p.feet)
assert.ok(p.crouch, 'and stays crouched while covered')

// a slide from a sprint carries past where a walk would stop
blocks.clear()
const a = run(newPlayer([0.5, 1, 0.5]), { fwd: 1, sprint: true }, 1)
const z0 = a.feet[2]
run(a, { slide: true }, 0.6)
assert.ok(a.slide > -1 && z0 - a.feet[2] > 4, 'slides on: ' + (z0 - a.feet[2]))
assert.ok(!hits(a.feet, P.hc, solid))
console.log('walk: ok')
