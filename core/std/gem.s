{std/gem.s  gematria: ordinal, reduced and reverse ordinal sums over letters a-z} //
0 \( _gem.s~ 1 \= \) <-?
0 \(  1 _gem.s#
std/text.s.

{a 1 b 2 c 3 d 4 e 5 f 6 g 7 h 8 i 9 j 10 k 11 l 12 m 13 n 14 o 15 p 16 q 17 r 18 s 19 t 20 u 21 v 22 w 23 x 24 y 25 z 26} _ordt`
{a 1 b 2 c 3 d 4 e 5 f 6 g 7 h 8 i 9 j 1 k 2 l 3 m 4 n 5 o 6 p 7 q 8 r 9 s 1 t 2 u 3 v 4 w 5 x 6 y 7 z 8} _redt`
{a 26 b 25 c 24 d 23 e 22 f 21 g 20 h 19 i 18 j 17 k 16 l 15 m 14 n 13 o 12 p 11 q 10 r 9 s 8 t 7 u 6 v 5 w 4 x 3 y 2 z 1} _rordt`

{_gc@ 0 \( {~_gt~} {~_gc~} '\0' \?) \)}                      '_gval' ><
{1 \( lower chars {_gval} map sum \)}                         '_gsum' ><
{0 \( {~_ordt~} _gt` \) _gsum}                                 'ord' ><   {text → ordinal sum, a=1 … z=26} //
{ord}                                                          '_gord' ><
{0 \( {~_redt~} _gt` \) _gsum}                                 'red' ><   {text → reduced sum, a=1 … i=9 j=1 …} //
{0 \( {~_rordt~} _gt` \) _gsum}                                'rord' ><  {text → reverse ordinal sum, a=26 … z=1} //
{_gw@ {~_gw~} {~_gw~} ord {~_gw~} red {~_gw~} rord}         'gem' ><   {text → text ord red rord} //
{1 \( words {_gord} map \)}                                      'ords' ><  {text → ord of each word} //

\)
