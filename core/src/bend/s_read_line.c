Term s_read_line_run(Env e, Term* f, IoWork* w) {
  static char* buf = NULL;
  static size_t cap = 0;
  fflush(stdout);
  ssize_t n = getline(&buf, &cap, stdin);
  if (n < 0) {
    return io_str(e, "0", 1);
  }
  while (n > 0 && (buf[n - 1] == '\n' || buf[n - 1] == '\r')) {
    n -= 1;
  }
  char* t = malloc((size_t)n + 1);
  t[0] = '1';
  memcpy(t + 1, buf, (size_t)n);
  Term r = io_str(e, t, (uint64_t)n + 1);
  free(t);
  return r;
}

static void __attribute__((constructor)) s_read_line_use(void) {
  io_eff(CID(S.read_line), s_read_line_run);
}
