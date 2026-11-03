{std/check.s  checks beside the code: each prints ok or what it got, and checked prints the tally} //
0 \( _check.s~ 1 \= \) <-?
0 \(  1 _check.s#
std/core.s.
0 _cp#  0 _cf#

{2 \( _cw@ _cg@ 0 \( _cp; \) ok \> \)}                                        '_ckok' ><
{2 \( _cw@ _cg@ 0 \( _cf; \) fail got {~_cg~} want {~_cw~} \> \)}          '_ckbad' ><
{_cw@ _cg@ {~_cg~} {~_cw~} same {~_cg~} {~_cw~} _ckok? _ckbad!}             'expect' ><   {got want → : ok when the same text} //
{_cw@ _cg@ {~_cg~} {~_cw~} eq {~_cg~} {~_cw~} _ckok? _ckbad!}               'expectn' ><  {got want → : ok when equal as numbers} //
{_cq@ {~_cq~} \~ yes? no! 1 0 drop? nip! 1 _ckb@ _cka@ {~_cka~} {~_ckb~} _ckok? _ckbad!}   'assert' ><  {… q → … : ok when q leaves ow true} //
{0 \( checked _cp~ ok _cf~ failed \> \) 0 _cp# 0 _cf#}                   'checked' ><  {: print the tally and start a new one} //

\)
