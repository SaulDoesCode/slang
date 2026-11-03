std/core.s.
1 2 3 dup > 
1 2 swap >  1 2 over >  1 2 3 rot >  1 2 nip >  1 2 tuck >  1 2 3 drop >  1 2 3 depth >
9 1 2 add >  9 7 2 sub >  9 3 4 mul >  9 1 4 div >  9 -7 3 mod >  9 2 10 pow >  9 5 neg >  9 5 inc dec dec >
-4 abs 4 abs >  3 8 min 3 8 max >
3 8 lt T? F! >  3 8 gt T? F! >  a a same T? F! >  2 2.0 eq T? F! > 
x 3 8 lt {small} {big} if >
x 9 8 lt {small} {big} if >
x 3 8 lt {small} when >  x 9 8 lt {small} when >  x 9 8 lt {big} unless >
{3 8 lt} {5 5 eq} and T? F! >  {3 8 lt} {5 6 eq} and T? F! >  {9 8 lt} {5 5 eq} or T? F! >
1 2 10 {inc} dip >   5 {dup mul} keep >  6 {inc} {dec} bi >
0 {inc} 5 times >
1 {2 mul} 10 times >
x {y} 0 times >
{1 {a} 3 times {b} 2 times} 2 times >
0 i#  {i~ 5 lt} {i~ >_ { } >_ i;} while >
1 {2 mul} {dup 100 gt} until >
