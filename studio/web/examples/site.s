{a site described in s: a nav, three pages, cards, a counter, a list that is kept} //

{s site} title

{
body { margin: 0; font: 16px/1.55 system-ui, sans-serif; background: #f4ece1; color: #3b2d20 }
nav { display: flex; gap: 18px; padding: 14px 28px; background: #fbf6ee; box-shadow: 0 2px 8px rgba(92,62,32,.15) }
nav a { color: #7a5532; text-decoration: none }
nav a:hover { text-decoration: underline }
main { max-width: 760px; margin: 28px auto; padding: 0 20px }
.cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 14px }
.card { background: #fbf6ee; padding: 14px 18px; box-shadow: 0 8px 20px -10px rgba(92,62,32,.4) }
.card h3 { margin: 0 0 6px; color: #7a5532 }
button { font: inherit; background: #7a5532; color: #fbf6ee; border: 0; padding: 6px 14px; cursor: pointer }
input { font: inherit; padding: 5px 8px; border: 1px solid #cfb08a; background: #fbf6ee }
.big { font-size: 2.2em; margin: 0 }
} sheet

{_cb@ _ch@ {{~_ch~} h3 {~_cb~} p} article.card}  Card ><

{
  {#/} href= {Home} a
  {#/counter} href= {Counter} a
  {#/list} href= {List} a
} nav

{
  {Pages described in s} h1
  {Each paragraph of this site is s: element words, modifiers, and words of your own.} p
  {
    {Element words} {The name of an element is a word that makes it, with its classes and id written after the tag.} Card
    {Modifiers} {Attributes and events stand before the element they shape.} Card
    {Names} {A brace that reads a name runs again when an event changes that name, nothing else does.} Card
  } div.cards
} home page

0 count#
{
  {Counter} h1
  {
    {~count~} p.big
    {count;} onclick {+1} button
    {0 count#} onclick {reset} button
  } section
} counter page

todos keep
{
  {A list that stays} h1
  {What next?} placeholder= {~draft~} value= input
  {{~draft~} todos l  {} draft`} onclick {Add} button
  {todos l-} onclick {Clear} button
  {todos {{~it~} li} each} ul
} list page

routes {} main
