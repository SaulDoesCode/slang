{a command per line: push, pop, add, show} //
{arg~ st l} push$
{st -l $~ >} pop$
{st -l $~ a#  st -l $~ a~ + =~ st l} add$
{st o l/ o~ >} show$
{{unknown} {~cmd~} >} unknown$
{v <_  eof~ 1 == <-?
 v~ ws /l  ws 0 cmd [l]  ws 1 arg [l]
 {push push. pop pop. add add. show show.} {~cmd~} {unknown.} ?)
 go.} go$
go.
