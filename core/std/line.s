{std/line.s  the line as an array: every word here acts on the whole line; k ( … ) narrows it} //
0 \( _line.s~ 1 \= \) <-?
0 \(  1 _line.s#
std/core.s.

{moving the line in and out of lists; _lv numbers nested uses} //
{\## _d@ 0 \( {~_d~} 0 \>> \) _drain1..?} _drain$
{_v@ 0 \( {~_v~} {_dl~_lv~} \l \) _drain..} _drain1$

{0 \( {_dl~_lv~} _N \#l {~_N~} 0 \>> \) _pour1..?} _pour$
{0 \( {_dl~_lv~} \-l _gk -1 _c \[l] {~$~} {~_c~} \l \) _pour..} _pour1$
{_k@ 0 \( _lv; {_dl~_lv~} \l- {~_k~} \l- {~_k~} _gk \l \) _drain.. _pour.. 0 \( _gk \-l _lv: \)}   'gather' ><  {… name → : the line becomes list name} //

{0 \( _sn -1 _c \[l] _si -1 _i \[l] {~_c~} _N \#l {~_i~} {~_N~} \<< \) _spill1..?} _spill$
{0 \( _sn -1 _c \[l] _si -1 _i \[l] {~_c~} {~_i~} _x \[l] {~_i~} 1 _I \+ {~_I~} _si -1 \l= \) {~_x~} _spill..} _spill1$
{_k@ 0 \( {~_k~} _sn \l 0 _si \l \) _spill.. 0 \( _sn \-l _si \-l \)}   'spill' ><  {name → … : list name onto the line, list kept} //

{_shw gather 0 \( _shw spill \> \) _shw spill}       'show' ><   {… → … : print the line, keep it} //
{\## _d@ \// {~_d~}}                                  'count' ><  {… → n} //

{map, filter, fold, scan} //
{0 \( {_dl~_lv~} _N \#l {~_N~} 0 \>> \) _map1..?} _map$
{0 \( _mq -1 _c \[l] {_dl~_lv~} \-l \) {~$~} 1 \( {~_c~} \~ \) _map..} _map1$
{_q@ 0 \( _lv; {_dl~_lv~} \l- {~_q~} _mq \l \) _drain.. _map.. 0 \( _mq \-l _lv: \)}   'map' ><  {x… q → q(x)… : q sees one value at a time} //

{0 \( {_dl~_lv~} _N \#l {~_N~} 0 \>> \) _filter1..?} _filter$
{0 \( _fq -1 _c \[l] {_dl~_lv~} \-l \) {~$~} {~$~} 1 \( {~_c~} \~ \// \) drop! _filter..} _filter1$
{_q@ 0 \( _lv; {_dl~_lv~} \l- {~_q~} _fq \l \) _drain.. _filter.. 0 \( _fq \-l _lv: \)}   'filter' ><  {x… q → x… : keep x where q leaves ow true} //

{0 \( {_dl~_lv~} _N \#l {~_N~} 0 \>> \) _fold1..?} _fold$
{0 \( _oq -1 _c \[l] {_dl~_lv~} \-l \) {~$~} {~_c~} \~ _fold..} _fold1$
{_q@ \## _d@ 0 \( {~_d~} 0 \== \) \<-? 0 \( _lv; {_dl~_lv~} \l- {~_q~} _oq \l \) _drain.. 0 \( {_dl~_lv~} \-l \) {~$~} _fold.. 0 \( _oq \-l _lv: \)}   'fold' ><  {x… q → r : q joins values left to right} //
{_q@ \## _d@ 0 \( {~_d~} 0 \== \) \<-? 0 \( _lv; {_dl~_lv~} \l- \) _drain.. {_dl~_lv~} spill 0 \( {_dl~_lv~} \l- _lv: \) {_swap ~_q~} fold}   'foldr' ><  {x… q → r : q joins values right to left} //
{_q@ {_over _swap ~_q~} fold}                                                       'scan' ><  {x… q → running results} //

{0 \( _lv; {_dl~_lv~} \l- \) _drain.. {_dl~_lv~} spill 0 \( {_dl~_lv~} \l- _lv: \)}   'rev' ><  {a b c → c b a} //
{0 {_add} fold}                                        'sum' ><      {x… → total} //
{1 {_mul} fold}                                        'product' ><  {x… → product} //
{{_max} fold}                                          'most' ><     {x… → largest} //
{{_min} fold}                                          'least' ><    {x… → smallest} //
{1 \( _n@ 0 {_inc _dup} {~_n~} times drop \)}           'iota' ><     {n → 1 2 … n} //
{1 \( _n@ 0 {_dup _inc} {~_n~} times drop \)}           'til' ><      {n → 0 1 … n-1} //
{_n@ \## _d@ 0 \( {~_d~} {~_n~} sub 0 max _k@ \) {~_k~} \( _trash gather \)}            'take' ><  {x… n → first n} //
{_n@ \## _d@ 0 \( {~_d~} {~_n~} sub 0 max _k@ \) {~_k~} \( _keep gather \) \// _keep spill}   'skip' ><  {x… n → all but the first n} //

{sort: insertion through the line itself} //
{0 \( {_dl~_lv~} _N \#l {~_N~} 0 \>> \) _sort1..?} _sort$
{0 \( {_dl~_lv~} \-l {~$~} _sx` \) _ins.. {~_sx~} _unst.. _sort..} _sort1$
{\## _d@ 0 \( {~_d~} 0 \>> \) _ins1..?} _ins$
{_y@ 0 \( {~_y~} {~_sx~} \>> \) {~_y~} _ins2..?} _ins1$
{_y@ 0 \( {~_y~} {_st~_lv~} \l \) _ins..} _ins2$
{0 \( {_st~_lv~} _N \#l {~_N~} 0 \>> \) _unst1..?} _unst$
{0 \( {_st~_lv~} \-l \) {~$~} _unst..} _unst1$
{0 \( _lv; {_dl~_lv~} \l- {_st~_lv~} \l- \) _drain.. _sort.. 0 \( _lv: \)}   'sort' ><  {x… → ascending} //

\)
