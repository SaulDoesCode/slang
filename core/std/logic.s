{std/logic.s  gates on bits for the logic dialect: each takes its inputs from the line and leaves one bit} //
std/core.s.
0 \( _logic.s~ 1 \= \) <-?
0 \(  1 _logic.s#

{bits} //
{abs 1 min}                            'bit' ><    {a → 1 if a≠0, else 0} //
{}                                     'buf' ><    {a → a} //
{neg inc}                              'not' ><    {a → ¬a} //
{mul}                                  'and' ><    {a b → a∧b} //
{add 1 min}                            'or' ><     {a b → a∨b} //
{add 2 mod}                            'xor' ><    {a b → a⊕b} //
{mul neg inc}                          'nand' ><   {a b → ¬(a∧b)} //
{add 1 min neg inc}                    'nor' ><    {a b → ¬(a∨b)} //
{add 2 mod neg inc}                    'xnor' ><   {a b → ¬(a⊕b)} //
