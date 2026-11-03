{the numbers on stdin, one a line: their count, sum, smallest and largest} //
0 n#  0 s#
{x <_  eof~ 1 == <-?
 n;  s~ x~ + =~ s#
 n~ 1 == x~ lo#? // n~ 1 == x~ hi#? //
 x~ lo~ << x~ lo#? //
 x~ hi~ >> x~ hi#? //
 loop.} loop$
loop.
n~ s~ lo~ hi~ >
