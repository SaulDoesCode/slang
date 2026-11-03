const s_in = { buf: "", eof: false };

function s_read_line() {
  const fs = require("fs");
  while (!s_in.eof && !s_in.buf.includes("\n")) {
    const b = Buffer.alloc(65536);
    let n = 0;
    try {
      n = fs.readSync(0, b, 0, 65536, null);
    } catch (err) {
      if (err.code === "EAGAIN") continue;
      n = 0;
    }
    if (n === 0) s_in.eof = true;
    else s_in.buf += b.toString("utf8", 0, n);
  }
  if (s_in.buf === "" && s_in.eof) return "0";
  const i = s_in.buf.indexOf("\n");
  let line;
  if (i < 0) {
    line = s_in.buf;
    s_in.buf = "";
  } else {
    line = s_in.buf.slice(0, i);
    s_in.buf = s_in.buf.slice(i + 1);
  }
  return "1" + str_in(line.replace(/\r+$/, ""));
}

io_eff(CID(S.read_line), s_read_line);
