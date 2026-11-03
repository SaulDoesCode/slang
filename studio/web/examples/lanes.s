{lanes: a line splits into lanes, each lane runs, the lanes join} //

⟪1¦2¦3¦4⟫ 10 \* =~ ⨝ {\+ =~} reduce total@
{Four lanes times ten, added at the join: ~total~.} p

{hello} msg`
⟪{~msg~} h1¦{~msg~} h2¦{~msg~} h3⟫ ⨝

{a red zone: try runs its first brace, and the second when the first fails} //
{{nope.} {{That failed: ~err~} p.err} try} section

{color: #b0413e} .err css
