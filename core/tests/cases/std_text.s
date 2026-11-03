std/text.s.
x foo bar cat >
x a b c concat >   concat >   a b c glue >   a b c {, } joinwith >
{hello there} quote >
{the quick  brown fox} words count >
{abc} chars >   {abc} len >   {} len >
{one,two,,three} {,} split >
{gematria} reversed >
{ab} 3 repeat >   {ab} 0 repeat >
{  lots   of    space } squish >
{memory base} {ory} has T? F! >   {memory base} {xyz} has T? F! >
{a-b-c} {-} {+} subst >   {banana} {an} occurs >
{Hello World!} upper >   {Hello World! lnrtwf} lower >   {lnrtwf} upper >
3 4 7 {% plus % is %} format >
{x}  {no args %} format >
5 {count: %%} format >
{ok} 1 2 {% and %, done
} printf
hello s say
