{std/math.s  numbers: rounding, signs, ranges, integers, means, chance} //
0 \( _math.s~ 1 \= \) <-?
0 \(  1 _math.s#
std/line.s.

{rounding} //
{1 \( 1 _mf \% _mf~ \)}                                   'frac' ><    {a → the part after the point, always ≥ 0} //
{_mx@ {~_mx~} {~_mx~} frac sub}                           'floor' ><   {a → largest whole ≤ a} //
{neg floor neg}                                           'ceil' ><    {a → smallest whole ≥ a} //
{0.5 add floor}                                           'round' ><   {a → nearest whole, halves up} //
{_mx@ {~_mx~} 0 lt {~_mx~} ceil? floor!}                  'trunc' ><   {a → whole part toward 0} //
{_md@ _mx@ 10 {~_md~} pow _mp@ {~_mx~} {~_mp~} mul round {~_mp~} div}   'places' ><  {a n → a rounded to n decimals} //

{signs and ranges} //
{_sx@ {~_sx~} 0 gt 1 0 drop? nip! {~_sx~} 0 lt 1 0 drop? nip! sub}      'sign' ><    {a → -1, 0 or 1} //
{_mh@ _ml@ {~_ml~} max {~_mh~} min}                       'clamp' ><   {a lo hi → a held within lo…hi} //
{_mh@ _ml@ _mx@ {~_mx~} {~_ml~} ge 1 0 drop? nip! {~_mx~} {~_mh~} le 1 0 drop? nip! mul 1 eq}   'within' ><  {a lo hi → : ow = lo ≤ a ≤ hi} //
{_mt@ _mb@ _ma@ {~_ma~} {~_mb~} {~_ma~} sub {~_mt~} mul add}   'lerp' ><    {a b t → a + (b - a) t} //
{0.5 pow}                                                 'sqrt' ><    {a → square root} //
{dup mul}                                                 'sq' ><      {a → a²} //
{sq swap sq add sqrt}                                     'hypot' ><   {a b → √(a² + b²)} //
{3.141592653589793}                                       'pi' ><      {→ π} //

{whole numbers} //
{2 mod 0 eq}                                              'even' ><    {n → : ow = n is even} //
{even not}                                                'odd' ><     {n → : ow = n is odd} //
{2 \( {0 ne} {_mb@ _ma@ {~_mb~} {~_ma~} {~_mb~} mod} while drop abs \)}   'gcd' ><  {a b → greatest common divisor} //
{_mc@ _mk@ {~_mk~} {~_mc~} mul abs {~_mk~} {~_mc~} gcd div}               'lcm' ><  {a b → least common multiple} //
{1 \( iota product \)}                                    'fact' ><    {n → n!} //
{_mn@ 0 1 {_mb@ _ma@ {~_mb~} {~_ma~} {~_mb~} add} {~_mn~} times drop}     'fib' ><  {n → the n-th Fibonacci number, 0 1 1 2 …} //
{1 \( _mn@ {~_mn~} iota {_md@ {~_mn~} {~_md~} mod 0 eq} filter \)}   'divisors' ><  {n → each divisor, ascending} //
{1 \( divisors count \) 2 eq}                         'prime' ><  {n → : ow = n is prime} //
{_mt@ _mf@ {~_mt~} {~_mf~} sub inc til {_ri@ {~_ri~} {~_mf~} add} map}   'range' ><   {a b → a a+1 … b} //

{the whole line} //
{\## _mn@ sum {~_mn~} div}                                'mean' ><    {x… → the mean} //
{sort _mv gather 0 \( _mv _mn \#l \) _mn~ 1 sub 2 div _mi@ 0 \( _mv {~_mi~} floor _ma \[l] _mv {~_mi~} ceil _mb \[l] _mv \l- \) {~_ma~} {~_mb~} add 2 div}   'median' ><  {x… → the middle value, or the mean of the two middle ones} //

{chance: one generator for the run; seed it to repeat a run} //
{0 \( \#prng rn~ \)}                                      'rand' ><    {→ a float in [0, 1)} //
{1 \( _mn@ rand {~_mn~} mul floor inc \)}                 'dice' ><    {n → a whole number 1 … n} //
{1 \( \#prng \// \)}                                      'seed' ><    {n → : the generator starts again from n} //

\)
