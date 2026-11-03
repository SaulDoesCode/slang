{std/text.s  text: build, split, join, case, search, format} //
0 \( _text.s~ 1 \= \) <-?
0 \(  1 _text.s#
std/line.s.

'{' _lb`  '}' _rb`  {
} _nl`

{_b@ _a@ {~_a~~_b~}}                                   'cat' ><     {a b → ab} //
{_cc gather 0 \( _cc _o {} \l/ \) {~_o~}}              'concat' ><  {x… → one text, no separators} //
{_cc gather 0 \( _cc _o \l/ \) {~_o~}}                 'glue' ><    {x… → one text, single spaces} //
{_j@ _cc gather 0 \( _cc _o {~_j~} \l/ \) {~_o~}}      'joinwith' >< {x… sep → one text} //
{_v@ {~_lb~~_v~~_rb~}}                                 'quote' ><   {x → code that pushes x} //

{_s@ 0 \( {~_s~} _sp \/l \) _sp spill}                 'words' ><   {text → word…} //
{_s@ 0 \( {~_s~} _sp {} \/l \) _sp spill}              'chars' ><   {text → char…} //
{_s@ 0 \( {~_s~} _sp {~_nl~} \/l _sp -1 _x \[l] {~_x~} {} \= _sp? \-l? \) _sp spill}   'lines' ><  {text → line… : a final newline ends the last line} //
{_j@ _s@ 0 \( {~_s~} _sp {~_j~} \/l \) _sp spill}      'split' ><   {text sep → part…} //
{1 \( _L \#s {~_L~} \)}                                'len' ><     {text → characters} //
{1 \( chars rev concat \)}                             'reversed' >< {text → txet} //
{_n@ _s@ 0 \( {~_s~} quote {~_n~} times concat \)}     'repeat' ><  {text n → texttext…} //
{1 \( words glue \)}                                   'squish' ><  {text → single-spaced} //

{_p@ _s@ 0 \( {~_s~} _hs {~_p~} \/l _hs _N \#l {~_N~} 1 \>> \)}              'has' ><    {text part → : ow = text contains part} //
{_r@ _p@ _s@ 0 \( {~_s~} _hs {~_p~} \/l _hs _o {~_r~} \l/ \) {~_o~}}         'subst' ><  {text old new → text with every old as new} //
{_p@ _s@ 0 \( {~_s~} _hs {~_p~} \/l _hs _N \#l \) {~_N~} dec}                 'occurs' >< {text part → how many times} //

{a 'A' b 'B' c 'C' d 'D' e 'E' f 'F' g 'G' h 'H' i 'I' j 'J' k 'K' l 'L' m 'M' n 'N' o 'O' p 'P' q 'Q' r 'R' s 'S' t 'T' u 'U' v 'V' w 'W' x 'X' y 'Y' z 'Z'} _upt`
{A 'a' B 'b' C 'c' D 'd' E 'e' F 'f' G 'g' H 'h' I 'i' J 'j' K 'k' L 'l' M 'm' N 'n' O 'o' P 'p' Q 'q' R 'r' S 's' T 't' U 'u' V 'v' W 'w' X 'x' Y 'y' Z 'z'} _lot`
{{~_ch~}}                                              '_chid' ><
{_ch@ 0 \( {~_upt~} {~_ch~} '_chid' \?) \)}            '_upc' ><
{_ch@ 0 \( {~_lot~} {~_ch~} '_chid' \?) \)}            '_loc' ><
{1 \( chars {_upc} map concat \)}                      'upper' ><   {text → TEXT} //
{1 \( chars {_loc} map concat \)}                      'lower' ><   {TEXT → text} //

{0 \( _fp _N \#l {~_FI~} {~_N~} \<< \) _format1..?} _format$
{0 \( {} _y` _fp {~_FI~} _x \[l] _fa {~_FI~} _y \[l] _FI; \) {~_x~} {~_y~} _format..} _format1$
{_f@ _fa gather 0 \( {~_f~} _fp {%} \/l 0 _FI# \) _format.. concat}       'format' ><  {x… text → text with each % filled in order} //
{format \>_}                                           'printf' ><  {x… text → : print it} //
{glue {~_nl~} cat \>_}                                 'say' ><     {x… → : print the line, then a newline} //

{records: name/key is an ordinary variable, name/keys lists the keys in the order they were first set} //
{_dk@ _dn@ {~_lb~~~~_dn~/~_dk~~~~_rb~} \~}                                      'get' ><   {name key → value, 404 when unset} //
{_dv@ _dk@ _dn@ 0 \( {~_dn~} {~_dk~} get {404} \= \) 0 \( {~_dk~} {~_dn~/keys} \l? \// \) 0 \( {~_dv~} {~_dn~/~_dk~`} \~ \)}   'set' ><  {name key value → : store} //
{_dn@ {~_dn~/keys} spill}                                                       'keys' ><  {name → key…} //

\)
