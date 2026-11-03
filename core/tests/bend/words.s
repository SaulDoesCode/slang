{for each line: its words, how many, and the words joined back with commas} //
{ln <_  eof~ 1 == <-?
 ln~ ws /l  ws c #l  ws j {,} l/
 c~ j~ >
 c~ 2 >> {long} >? //
 more.} more$
more.
