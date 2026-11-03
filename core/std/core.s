{std/core.s  stack words, line arithmetic, conditions, quotations, loops} //
0 \( _core.s~ 1 \= \) <-?
0 \(  1 _core.s#

{stack} //
{_a@}                                  'drop' ><   {a →} //
{_a@ {~_a~} {~_a~}}                    'dup' ><    {a → a a} //
{_a@ _b@ {~_a~} {~_b~}}                'swap' ><   {a b → b a} //
{_a@ _b@ {~_b~} {~_a~} {~_b~}}         'over' ><   {a b → a b a} //
{_c@ _b@ _a@ {~_b~} {~_c~} {~_a~}}     'rot' ><    {a b c → b c a} //
{_a@ _b@ {~_a~}}                       'nip' ><    {a b → b} //
{_b@ _a@ {~_b~} {~_a~} {~_b~}}         'tuck' ><   {a b → b a b} //
{\//}                                  'clear' ><  {… →} //
{\##}                                  'depth' ><  {… → … n} //

{line arithmetic: the top two values become one, the rest of the line stays} //
{2 \( _R \+ _R~ \)}                    'add' ><    {a b → a+b} //
{2 \( _R \- _R~ \)}                    'sub' ><    {a b → a-b} //
{2 \( _R \* _R~ \)}                    'mul' ><    {a b → a×b} //
{2 \( _R \/ _R~ \)}                    'div' ><    {a b → a÷b} //
{2 \( _R \% _R~ \)}                    'mod' ><    {a b → a mod b} //
{2 \( _R \^ _R~ \)}                    'pow' ><    {a b → a^b} //
{1 \( -1 _R \* _R~ \)}                 'neg' ><    {a → -a} //
{1 \( 1 _R \+ _R~ \)}                  'inc' ><    {a → a+1} //
{1 \( 1 _R \- _R~ \)}                  'dec' ><    {a → a-1} //
{1 \( _a@ {~_a~} 0 \<< {~_a~} neg? \)}                                    'abs' ><  {a → |a|} //
{2 \( _b@ _a@ {~_a~} {~_b~} \<< {~_a~} {~_b~} drop? nip! \)}              'min' ><  {a b → smaller} //
{2 \( _b@ _a@ {~_a~} {~_b~} \>> {~_a~} {~_b~} drop? nip! \)}              'max' ><  {a b → larger} //

{conditions: the top two values are compared and removed, ow holds the answer} //
{2 \( \<< \)}                          'lt' ><     {a b → : ow = a<b} //
{2 \( \>> \)}                          'gt' ><     {a b → : ow = a>b} //
{2 \( \<<= \)}                         'le' ><     {a b → : ow = a≤b} //
{2 \( \>>= \)}                         'ge' ><     {a b → : ow = a≥b} //
{2 \( \== \)}                          'eq' ><     {a b → : ow = a=b as numbers} //
{2 \( \== \!! \)}                      'ne' ><     {a b → : ow = a≠b as numbers} //
{2 \( \= \)}                           'same' ><   {a b → : ow = same text} //
{2 \( \!= \)}                          'differ' >< {a b → : ow = different text} //
{\!!}                                  'not' ><    {: ow = not ow} //
{0 \( 0 0 \== \)}                      'yes' ><    {: ow = true} //
{0 \( 1 0 \== \)}                      'no' ><     {: ow = false} //

{quotations: a brace literal is code waiting; ~ runs it on the line} //
{\~}                                   'call' ><   {… q → …} //
{drop? nip! \~}                        'if' ><     {… t f → … : run t when ow, else f} //
{{} if}                                'when' ><   {… q → … : run q when ow} //
{{} swap if}                           'unless' >< {… q → … : run q unless ow} //
{_t@ \## _d@ 0 \( {~_d~} 0 \>> \) dup? 1 0 drop? nip! \( {~_t~} \~ \// \)}   'test' ><   {… q → … : run q on a copy of the top value; only its ow remains} //
{_q2@ _q1@ {~_q2~} {~_q1~} test 1 \( when \// \)}    'and' ><  {p q → : ow = p and q, q tested only after p} //
{_q2@ _q1@ {~_q2~} {~_q1~} test 1 \( unless \// \)}  'or' ><   {p q → : ow = p or q} //
{_q@ _x@ 0 \( {~_x~} _dip \l \) {~_q~} \~ 0 \( _dip \-l \) {~$~}}       'dip' ><   {… x q → … x : run q under x} //
{_q@ dup {~_q~} dip}                                                   'keep' ><  {x q → q(x) x} //
{_q@ _p@ {~_q~} swap {~_p~} keep rot \~}                               'bi' ><    {x p q → p(x) q(x)} //

{loops: control state lives in lists, so loops nest} //
{0 \( _tn -1 _k \[l] {~_k~} 0 \>> \) _times1..?} _times$
{0 \( {~_k~} 1 _K \- {~_K~} _tn -1 \l= _tq -1 _c \[l] {~_c~} \) \~ _times..} _times1$
{_n@ _q@ 0 \( {~_q~} _tq \l {~_n~} _tn \l \) _times.. 0 \( _tq \-l _tn \-l \)}    'times' ><  {… q n → … : run q n times on the line} //

{0 \( _wc -1 _c \[l] {~_c~} \) test _while1..?} _while$
{0 \( _wb -1 _c \[l] {~_c~} \) \~ _while..} _while1$
{_b@ _c@ 0 \( {~_c~} _wc \l {~_b~} _wb \l \) _while.. 0 \( _wc \-l _wb \-l \)}    'while' ><  {… c b → … : while c tests true run b} //

{_c@ _b@ 0 \( {~_b~} _uq \l {~_c~} _uc \l \) _untilgo.. 0 \( _uq \-l _uc \-l \)}  'until' ><  {… b c → … : run b, then again until c tests true} //
{0 \( _uq -1 _c \[l] {~_c~} \) \~ 0 \( _uc -1 _c \[l] {~_c~} \) test _untilgo..!} _untilgo$

{private names for the library's own quotations, which run where they are called} //
{_a@ {~_a~} {~_a~}}                    '_dup' ><
{_a@ _b@ {~_a~} {~_b~}}                '_swap' ><
{_a@ _b@ {~_b~} {~_a~} {~_b~}}         '_over' ><
{2 \( _R \+ _R~ \)}                    '_add' ><
{2 \( _R \* _R~ \)}                    '_mul' ><
{1 \( 1 _R \+ _R~ \)}                  '_inc' ><
{2 \( _b@ _a@ {~_a~} {~_b~} \<< {~_a~} {~_b~} drop? nip! \)}              '_min' ><
{2 \( _b@ _a@ {~_a~} {~_b~} \>> {~_a~} {~_b~} drop? nip! \)}              '_max' ><
{2 \( _R \- _R~ \)}                    '_sub' ><
{2 \( _R \/ _R~ \)}                    '_div' ><

\)
