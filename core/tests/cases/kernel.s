{frames: k ( keeps the top k values, ) puts the rest back underneath} //
9 8 3 4 2 (  + =~ )  >
1 2 3 0 (  ## )  >
x y z 1 (  q@  )  q~ >

{pop and depth} //
p q s  top@  ##  >  top~ >

{eval runs the top value as code on the rest of the line} //
2 3 {+ =~} ~  >
{shout} {>} ~

{a replacement runs as code where the word stands} //
{2 ( + =~ )} add ><
1 2 3 add add  >
{> } say ><
hi there say

{a replacement can rename a command; the escape reaches the original; a defined word is quoted when named as the key} //
{2 ( \+ =~ )} '+' ><
10 20 30 + +  >
5 6 \+  =~ >

{inside its own body a replacement is the original: no self-loop} //
{\> {(wrapped)} \>} shout ><
{ {[} shout {]} > } ~
{{<} > >} '>' ><
x >
{done} \>

{a body sees replacements older than itself} //
{a} first ><
{first first} pair ><
{b} 'first' ><
pair \>
first \>

{scripts see replacements that existed when they were defined} //
{late \>} early$
{LATE} late ><
early.
{late \>} after$
after.

{looked-up values run as words} //
{x@} popper`
7 8 popper~  ## \>  x~ \>
{n~ \>} show$  {show.} runner`
5 n#  runner~
'\>' raw`  q u raw~

{numbers can be replaced too} //
{seven} 7 ><
7 \>
{7} \>
3 4 \+ =~ \>
