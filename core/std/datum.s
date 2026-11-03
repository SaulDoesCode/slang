{std/datum.s  datum cubes: a value of 24 bits, red green blue, and its bits by number} //
std/core.s.
0 \( _datum.s~ 1 \= \) <-?
0 \(  1 _datum.s#

{bytes} //
{dup 65536 mod sub 65536 div}          'red' ><    {v → red byte} //
{65536 mod dup 256 mod sub 256 div}    'green' ><  {v → green byte} //
{256 mod}                              'blue' ><   {v → blue byte} //
{swap 256 mul add swap 65536 mul add}  'rgb' ><    {r g b → v} //

{bits} //
{2 swap pow div dup 1 mod sub 2 mod}   'bitof' ><  {v n → bit n of v} //
{_n@ _v@ {~_v~} {~_v~} {~_n~} bitof 2 mul neg inc 2 {~_n~} pow mul add}   'flip' ><   {v n → v with bit n toggled} //
