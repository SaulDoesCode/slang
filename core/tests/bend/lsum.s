{two thousand numbers into a list, then their sum read back by index} //
0 i#
{i~ 2000 == <-?
 i~ 2 * =~ xs l
 i;  fill.} fill$
fill.
0 s#  0 j#
{j~ 2000 == <-?
 xs j~ v [l]  s~ v~ + =~ s#
 j;  sum.} sum$
sum.
s~ >
