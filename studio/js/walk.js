// Walking: a player box moving through the grid with gravity, steps, vaults, crouching
// and slides. Pure functions over a `solid(x, y, z)` test of grid cells, so it runs and
// is tested without a scene.

export const P = {
  r: 0.3, // half width
  h: 1.75, // standing height
  hc: 0.9, // crouched height: fits under one cell
  eye: 1.62,
  eyec: 0.75,
  walk: 4.6,
  sprint: 8.4,
  crouch: 2.2,
  accGround: 46,
  accAir: 11,
  gravity: 24,
  jump: 7.4,
  step: 0.55, // climbed without jumping
  vault: 1.15, // climbed with a jump or while airborne
  slideBoost: 3.2,
  slideTime: 0.85,
  slideDrag: 1.4,
}

// the body's size in cells: at 1 a brick is a ledge to vault, larger makes the code
// bench-sized and the player tall over it, as a board is under a hand
const LENGTHS = ['r', 'h', 'hc', 'eye', 'eyec', 'walk', 'sprint', 'crouch', 'accGround', 'accAir', 'gravity', 'jump', 'step', 'vault', 'slideBoost']
const BASE = { ...P }
export function scaleBody(k) {
  // speeds, accelerations and gravity all scale with k, so jumps and slides keep their shape
  for (const n of LENGTHS) P[n] = BASE[n] * k
}

export function newPlayer(feet) {
  return { feet: [...feet], vel: [0, 0, 0], ground: false, crouch: false, slide: 0, jumpHeld: false }
}

export function eyeHeight(p) {
  return p.crouch ? P.eyec : P.eye
}

// whether the player's box with its feet at f and height h overlaps a solid cell
export function hits(f, h, solid) {
  const e = 1e-6
  const x0 = Math.floor(f[0] - P.r + e), x1 = Math.floor(f[0] + P.r - e)
  const y0 = Math.floor(f[1] + e), y1 = Math.floor(f[1] + h - e)
  const z0 = Math.floor(f[2] - P.r + e), z1 = Math.floor(f[2] + P.r - e)
  for (let x = x0; x <= x1; x++) for (let y = y0; y <= y1; y++) for (let z = z0; z <= z1; z++) if (solid(x, y, z)) return true
  return false
}

// inp: { fwd, right (-1…1), yaw (radians, 0 looks along -z), jump, sprint, crouch, slide }
export function stepPlayer(p, inp, dt, solid) {
  // standing up needs the room for it
  if (inp.crouch || p.slide > 0) p.crouch = true
  else if (p.crouch && !hits(p.feet, P.h, solid)) p.crouch = false
  const h = p.crouch ? P.hc : P.h
  const s = Math.sin(inp.yaw), c = Math.cos(inp.yaw)
  let wx = -s * inp.fwd + c * inp.right
  let wz = -c * inp.fwd - s * inp.right
  const wl = Math.hypot(wx, wz)
  if (wl > 1) { wx /= wl; wz /= wl }
  const hs = Math.hypot(p.vel[0], p.vel[2])
  // a slide starts from a sprint on the ground and carries the speed on
  if (inp.slide && p.ground && p.slide <= 0 && hs > P.walk * 1.05) {
    p.slide = P.slideTime
    p.vel[0] += (p.vel[0] / hs) * P.slideBoost
    p.vel[2] += (p.vel[2] / hs) * P.slideBoost
    p.crouch = true
  }
  if (p.slide > 0) {
    p.slide -= dt
    const k = Math.max(0, 1 - P.slideDrag * dt)
    p.vel[0] *= k
    p.vel[2] *= k
  } else {
    const speed = p.crouch ? P.crouch : inp.sprint ? P.sprint : P.walk
    const acc = (p.ground ? P.accGround : P.accAir) * dt
    const dx = wx * speed - p.vel[0], dz = wz * speed - p.vel[2]
    const dl = Math.hypot(dx, dz)
    const f = dl > acc ? acc / dl : 1
    p.vel[0] += dx * f
    p.vel[2] += dz * f
  }
  if (inp.jump && !p.jumpHeld && p.ground) {
    p.vel[1] = P.jump
    p.ground = false
    p.slide = 0
  }
  p.jumpHeld = !!inp.jump
  p.vel[1] -= P.gravity * dt
  // horizontal moves, one axis at a time, stepping up or vaulting onto ledges
  for (const ax of [0, 2]) {
    const d = p.vel[ax] * dt
    if (!d) continue
    const n = [...p.feet]
    n[ax] += d
    if (!hits(n, h, solid)) { p.feet = n; continue }
    const limit = p.ground && !inp.jump ? P.step : P.vault
    let climbed = false
    for (let top = Math.floor(p.feet[1]) + 1; top - p.feet[1] <= limit + 1e-9; top++) {
      const up = [n[0], top, n[2]]
      if (!hits(up, h, solid) && !hits([p.feet[0], top, p.feet[2]], h, solid)) {
        p.feet = up
        if (p.vel[1] < 0) p.vel[1] = 0
        climbed = true
        break
      }
    }
    if (!climbed) p.vel[ax] = 0
  }
  // vertical
  const ny = p.feet[1] + p.vel[1] * dt
  const n = [p.feet[0], ny, p.feet[2]]
  if (!hits(n, h, solid)) {
    p.feet = n
    p.ground = p.vel[1] <= 0 && hits([p.feet[0], p.feet[1] - 0.04, p.feet[2]], h, solid)
  } else if (p.vel[1] < 0) {
    p.feet[1] = Math.floor(ny) + 1
    while (hits(p.feet, h, solid)) p.feet[1] += 1
    p.vel[1] = 0
    p.ground = true
  } else {
    p.feet[1] = Math.floor(ny + h) - h
    p.vel[1] = 0
  }
  if (p.ground) {
    // friction when no input on the ground
    if (!wl && p.slide <= 0) {
      const k = Math.max(0, 1 - 14 * dt)
      p.vel[0] *= k
      p.vel[2] *= k
    }
  }
  return p
}
