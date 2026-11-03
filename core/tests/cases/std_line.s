std/line.s.
a b c xs gather  xs #l =~ >  xs spill >  
1 2 3 show > 
1 2 3 4 count >
1 2 3 {2 mul} map >
1 2 3 {dup} map >
1 2 3 4 5 6 {2 mod 0 eq} filter >
1 2 3 4 {add} fold >   {add} fold >   7 {add} fold >
1 2 3 {sub} fold >  1 2 3 {sub} foldr >
1 2 3 4 {add} scan >
1 2 3 rev >   rev >
1 2 3 4 sum >  sum >   1 2 3 4 product >  3 9 2 most >  3 9 2 least >
5 iota >  0 iota >  5 til >  
1 2 3 4 5 2 take >  1 2 3 4 5 2 skip >  1 2 9 take >  1 2 9 skip >
5 3 9 1 7 3 sort >  sort > 
x y 1 3 4 2 ( sum ) >
10 iota {2 mod 0 eq} filter {dup mul} map sum >
3 iota {1 ( iota sum )} map >
