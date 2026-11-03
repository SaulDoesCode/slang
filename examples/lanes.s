{1 5 9 3 4 12 0 7 8} xs /l

{vector programs: numbers on the lanes, and after the merge only arithmetic, comparisons and writes under the flag} //
⟪xs…⟫ 3 * =~ 1 + =~ ⨝ ys into  ys o l/ o~ >
⟪xs…⟫ v# v~ 4 >> 4 v#? // v~ ⨝ ys into  ys o l/ o~ >
⟪xs…⟫ x# x~ 2 % =~ 0 == 1 x;? // x~ ⨝ ys into  ys o l/ o~ >
⟪1¦2¦3¦4⟫ 10 * =~ q# ⨝  2 lane q~ >  endlane

{a million lanes, one vector program} //
{n~ big l} fill$  1000000 fill &&&
⟪big…⟫ 2 * =~ 1 + =~ ⨝ out into  out -1 last [l]  last~ >

{threads: each lane holds code} //
⟪{a} >¦{b} >¦{c} >⟫ ⨝

{one lane at a time: a lane prints} //
⟪xs…⟫ v# v~ > ⨝
