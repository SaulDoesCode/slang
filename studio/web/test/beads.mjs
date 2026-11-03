// node studio/web/test/beads.mjs: the tests of BEADS.md §5 that the web machine covers,
// and try.
import { Rt } from '../s.js'
import { LIB } from '../std.js'
import { expand, mutualize, lineStats } from '../beads.js'
import { codes, encode, decode } from '../s5.js'
import fs from 'node:fs'
import path from 'node:path'
let ok = 0, bad = 0
const eq = (name, got, want) => {
  const g = JSON.stringify(got), w = JSON.stringify(want)
  if (g === w) ok++
  else { bad++; console.log('--- ' + name + '\n  want: ' + w + '\n  got:  ' + g) }
}
function run(src) {
  let out = ''
  const rt = new Rt({ lib: LIB, write: (s) => (out += s) })
  rt.setArgv([])
  const st = rt.runText(src)
  return st && st.error ? out + ' !' + st.error : out
}

// 5.1 s5 round trip
const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), '../../..')
let trips = 0, fails = []
for (const d of ['core/std', 'core/tests/cases', 'studio/web', 'studio/web/examples']) {
  for (const f of fs.readdirSync(path.join(root, d))) {
    const p = path.join(root, d, f)
    if (!fs.statSync(p).isFile() || /\.min\.js$|s-web\.js|s-core\.js/.test(f)) continue
    const t = fs.readFileSync(p, 'utf8')
    trips++
    if (decode(encode(t)) !== t) fails.push(f)
  }
}
eq('5.1 round trip of ' + trips + ' texts', fails, [])
// 5.2 CAP
eq('5.2 Hi', [codes('Hi'), decode(encode('Hi'))], [[27, 8, 9], 'Hi'])
eq('5.2 HI t', [codes('HI t'), decode(encode('HI t'))], [[27, 27, 8, 9, 0, 20], 'HI t'])
// 5.3 UNI and BEAD
eq('5.3 UNI', [codes('⟨').slice(0, 2), codes('⟨').length, decode(encode('⟨'))], [[30, 3], 5, '⟨'])
eq('5.3 BEAD', codes('⟪¦⟫⨝'), [31, 0, 31, 1, 31, 2, 31, 3])
// 5.4 expansion
eq('5.4 zip', expand('the ⟪cat¦dog⟫ sat on the ⟪mat¦log⟫'), ['the cat sat on the mat', 'the dog sat on the log'])
eq('5.4 product', expand('the ⟪a:cat¦dog⟫ sat on the ⟪b:mat¦log⟫'), ['the cat sat on the mat', 'the cat sat on the log', 'the dog sat on the mat', 'the dog sat on the log'])
eq('5.4 join', expand('⟪we¦they⟫ ran ⨝ home'), ['we ran home', 'they ran home'])
// 5.5 zip error, and labels
let msg = ''
try { expand('⟪a¦b⟫ ⟪c¦d¦e⟫') } catch (e) { msg = e.message + ' @' + e.at }
eq('5.5 zip error', msg, 'this ⟪ has 3 lanes where its set has 2 @6')
eq('5.5 labelled', expand('⟪x:a¦b⟫ ⟪y:c¦d¦e⟫').length, 6)
// 5.6 SIMD
eq('5.6 collect', run('⟪1¦2¦3¦4⟫ 10 \\+ =~ ⨝ collect >'), '\n 11 12 13 14')
eq('5.6 reduce', run('⟪1¦2¦3¦4⟫ 10 \\+ =~ ⨝ {\\+ =~} reduce >'), '\n 50')
eq('5.6 ascii', run('[[ 1 || 2 ]] 10 \\+ =~ ||> collect >'), '\n 11 12')
// 5.7 writes filed by lane
eq('5.7 home', run('5 x# ⟪1¦2⟫ x# ⨝ x~ >'), '\n 5')
eq('5.7 lane', run('5 x# ⟪1¦2⟫ x# ⨝ 1 lane x~ >'), '\n 2')
eq('5.7 endlane', run('5 x# ⟪1¦2⟫ x# ⨝ 1 lane endlane x~ >'), '\n 5')
// 5.8 output order
eq('5.8', run('⟪{a} >¦{b} >⟫ ⨝'), '\n a\n b')
// 5.9 lanes after join
eq('5.9 lanes', run('⟪1¦2¦3⟫ ⨝ lanes >'), '\n 3')
eq('5.9 lanewise', run('⟪1¦2¦3⟫ ⨝ {2 \\* =~} lanewise collect >'), '\n 2 4 6')
// product of sets in code, a fork with no join, nesting
eq('sets', run('⟪a:1¦2⟫ ⟪b:10¦20⟫ \\* =~ ⨝ collect >'), '\n 10 20 20 40')
eq('to the end of the line', run('⟪1¦2⟫ 3 \\+ =~\ncollect >'), '\n 4 5')
eq('nested', run('⟪⟪10¦20⟫ 1 \\+ =~ ⨝ {\\+ =~} reduce ¦ 5⟫ ⨝ collect >'), '\n 32 5')
eq('lanes in a replacement', run('{⟪1¦2⟫ 3 \\* =~ ⨝ collect} three ><  three >'), '\n 3 6')
eq('std still loads', run('std/all.s. 3 4 add >'), '\n 7')
eq('error names the lane', run('⟪1¦2⟫ 2 \\= <-! nope. ⨝'), ' !lane 1: nope.: no script or file named \'nope\'')
eq('stray mark', run('1 ¦ 2'), ' !parse: at 2: ¦ outside a fork')
// spread forks and into
eq('spread', run('{1 2 3 4} xs /l ⟪xs…⟫ 10 \\* =~ ⨝ ys into ys o l/ o~ >'), '\n 10 20 30 40')
eq('spread ascii', run('{1 2 3} xs /l ⟪xs...⟫ 2 \\^ =~ ⨝ {\\+ =~} reduce >'), '\n 14')
eq('spread text', run('{a b} xs /l ⟪xs…⟫ v@ {~v~!} ⨝ collect >'), '\n a! b!')
// every lane starts with the flag as at the fork, and the join puts it back
eq('flag per lane', run('0 v# 1 1 \\= {1 2 3} xs /l ⟪xs…⟫ 5 v#? // 3 2 \\<< v~ ⨝ collect >'), '\n 1 2 3')
eq('flag after the join', run('1 1 \\= ⟪1¦2⟫ 3 2 \\<< ⨝ {yes} >? {no} >!'), '\n yes')
// try
eq('try ok', run('{1 >} {caught >} try 2 >'), '\n 1\n 2')
eq('try catches', run('{1 > nope. 2 >} {{caught:} err~ >} try 3 >'), "\n 1\n caught: nope.: no script or file named 'nope'\n 3")
eq('try in a word', run('{{nope.} {err~ >} try {after} >} ww ><  ww {end} >'), "\n nope.: no script or file named 'nope'\n after\n end")
eq('try around lanes', run('5 x# {⟪1¦2⟫ x# nope. ⨝} {x~ >} try'), '\n 5')
// mutualize, stats
eq('mutualize', mutualize(['the cat sat on the mat', 'the dog sat on the log']), 'the ⟪cat¦dog⟫ sat on the ⟪mat¦log⟫')
eq('mutualize start', mutualize(['we ran home', 'they ran home']), '⟪we¦they⟫ ran home')
eq('stats', lineStats('the ⟪a:cat¦dog⟫ sat on the ⟪b:mat¦log⟫'), { forks: 2, lanes: 4, minted: 4 })
console.log(ok + ' pass, ' + bad + ' fail')
