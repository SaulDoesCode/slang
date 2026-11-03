{std/sh.s  pipelines over lines of text: the line holds one value per text line, |> is a separator you can read along} //
0 \( _sh.s~ 1 \= \) <-?
0 \(  1 _sh.s#
std/text.s.

{}                                                  '|>' ><     {: separator, does nothing} //
{}                                                  '~>' ><     {: the same separator after a script body turns | into ~} //
{_p@ 0 \( {~_p~} _fc \r \) {~_fc~} lines}          'cat' ><    {path → line…} //
{_c@ 0 \( {~_c~} \< \) {~r~} lines}                 'sh' ><     {command → its output lines} //
{_g@ {~_lb~~_g~~_rb~ has} filter}                   'grep' ><   {line… text → lines that contain text} //
{_g@ {~_lb~~_g~~_rb~ has not} filter}               'grepv' ><  {line… text → lines that do not} //
{_sn@ _so@ {~_lb~~_so~~_rb~ ~_lb~~_sn~~_rb~ subst} map}   'sed' ><  {line… old new → each line with old as new} //
{take}                                              'head' ><   {line… n → first n} //
{_n@ {~_n~} \( _tl gather \) \// _tl spill}         'tail' ><   {line… n → last n} //
{count}                                             'wc' ><     {line… → how many} //
{0 \( {} _uprev` \) {_ux@ {~_ux~} {~_uprev~} \!= {~_ux~} _uprev`} filter}   'uniq' ><  {line… → without repeats next to each other} //
{0 \( 0 _nln# \) {_nx@ 0 \( _nln; \) {~_nln~  ~_nx~}} map}                  'nl' ><    {line… → numbered lines} //
{{upper} map}                                       'shout' ><  {line… → each in capitals} //
{\## _d@ 0 \( {~_d~} 0 \== \) \<-? _sh gather 0 \( _sh _o {~_nl~} \l/ {~_o~~_nl~} \>_ \)}                        'put' ><   {line… → : print one per line} //
{_p@ _sh gather 0 \( _sh _o {~_nl~} \l/ {~_p~} {~_o~~_nl~} \w \)}              '>file' ><  {line… path → : write them to path} //
{_p@ _sh gather 0 \( _sh _o {~_nl~} \l/ {~_p~} {~_o~~_nl~} \+w \)}             '>>file' >< {line… path → : append them to path} //

\)
