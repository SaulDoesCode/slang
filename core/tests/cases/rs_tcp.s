{a server and a client in one program: the client is a worker} //
127.0.0.1:0 srv s tcp
rs_tcp_client.s t
{127.0.0.1:~port~ k c tcp  k {hello from the client} f_  k reply _f  {'client got: ~reply~' >} out`  k f-} 0 <t
srv conn a tcp
conn line _f  {server got: ~line~} >
conn {echo ~line~} f_
0 >t
conn line _f  conn ?eof  {after close, eof is ~eof~} >
conn f-  srv f-
127.0.0.1:1 x c {{connection refused, as expected} >} tcp
