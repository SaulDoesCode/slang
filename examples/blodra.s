{a state is side.way: sides t f b l r u, ways t l b r; each state lists the states top left bottom right lead to} //

b.b l.l f.t r.r  t.t l
l.b f.l r.t b.r  t.l l
f.b r.l b.t l.r  t.b l
r.b b.l l.t f.r  t.r l

t.t l.t u.t r.t  f.t l
l.l u.l r.l t.l  f.l l
u.b r.b t.b l.b  f.b l
r.r t.r l.r u.r  f.r l

t.b r.t u.b l.t  b.t l
r.l u.r l.l t.r  b.l l
u.t l.b t.t r.b  b.b l
l.r t.l r.r u.l  b.r l

t.r b.t u.l f.t  l.t l
b.l u.b f.l t.t  l.l l
u.r f.b t.l b.b  l.b l
f.r t.b b.r u.t  l.r l

t.l f.t u.r b.t  r.t l
f.l u.t b.l t.b  r.l l
u.l b.b t.r f.b  r.b l
b.r t.t f.r u.b  r.r l

f.t l.r b.b r.l  u.t l
l.t b.r r.b f.l  u.l l
b.t r.r f.b l.l  u.b l
r.t f.r l.b b.l  u.r l

{st~ 0 st [l]  {~path~A} path`} 'top' ><
{st~ 1 st [l]  {~path~D} path`} 'left' ><
{st~ 2 st [l]  {~path~C} path`} 'bottom' ><
{st~ 3 st [l]  {~path~B} path`} 'right' ><
{top} 'A' ><  {right} 'B' ><  {bottom} 'C' ><  {left} 'D' ><

{0 ( st~ 0 _p [l]  _p~ 0 _p [l] ) _p~} '$top' ><
{0 ( st~ 1 _p [l]  _p~ 1 _p [l] ) _p~} '$left' ><
{0 ( st~ 2 _p [l]  _p~ 2 _p [l] ) _p~} '$bottom' ><
{0 ( st~ 3 _p [l]  _p~ 3 _p [l] ) _p~} '$right' ><

{{~side~.t} st` {A} path`} 'start.A' ><
{{~side~.l} st` {D} path`} 'start.D' ><
{{~side~.b} st` {C} path`} 'start.C' ><
{{~side~.r} st` {B} path`} 'start.B' ><

{side@ :p@  {~:p~} :cs {} /l  :cs :c l/  {start.~:c~} ~} 'walk' ><
{st~ :r l/  $top :a@ $left :b@ $bottom :c@ $right :d@  {~path~} :r~ :a~ :b~ :c~ :d~ >} 'show' ><
