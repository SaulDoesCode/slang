#include <sys/wait.h>
#include <fcntl.h>
#include <unistd.h>

static uint64_t s_rng[4];

#define S_FILES 256
static struct { char* name; FILE* fp; int eof; } s_files[S_FILES];

static int s_find(const char* name) {
  for (int i = 0; i < S_FILES; i++) if (s_files[i].name && strcmp(s_files[i].name, name) == 0) return i;
  return -1;
}
static void s_drop(const char* name) {
  int i = s_find(name);
  if (i >= 0) { fclose(s_files[i].fp); free(s_files[i].name); s_files[i].name = NULL; }
}
static int s_rng_set = 0;

static uint64_t s_splitmix(uint64_t* z) {
  *z += 0x9E3779B97F4A7C15ull;
  uint64_t x = *z;
  x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9ull;
  x = (x ^ (x >> 27)) * 0x94D049BB133111EBull;
  return x ^ (x >> 31);
}

static void s_seed(uint64_t a, uint64_t b) {
  uint64_t z = a ^ ((b << 32) | (b >> 32));
  for (int i = 0; i < 4; i++) s_rng[i] = s_splitmix(&z);
  s_rng_set = 1;
}

static uint64_t s_rotl(uint64_t x, int k) { return (x << k) | (x >> (64 - k)); }

static uint64_t s_next(void) {
  if (!s_rng_set) {
    uint64_t a = 0, b = 0;
    int fd = open("/dev/urandom", O_RDONLY);
    if (fd >= 0) { if (read(fd, &a, 8) != 8) a = 1; if (read(fd, &b, 8) != 8) b = 2; close(fd); }
    s_seed(a, b);
  }
  uint64_t* s = s_rng;
  uint64_t r = s_rotl(s[1] * 5, 7) * 9;
  uint64_t t = s[1] << 17;
  s[2] ^= s[0]; s[3] ^= s[1]; s[1] ^= s[2]; s[0] ^= s[3];
  s[2] ^= t; s[3] = s_rotl(s[3], 45);
  return r;
}

static void s_seed_from(const char* b) {
  if (b[0] == 0) return;
  double x = 0, y = 0;
  int n = sscanf(b, "%lf %lf", &x, &y);
  s_seed((uint64_t)(int64_t)x, n > 1 ? (uint64_t)(int64_t)y : 0);
}

static Term s_ok(Env e, const char* p, size_t n) {
  char* t = malloc(n + 1);
  t[0] = '1';
  memcpy(t + 1, p, n);
  Term r = io_str(e, t, n + 1);
  free(t);
  return r;
}

static Term s_no(Env e, const char* m0) {
  char m[256];
  snprintf(m, sizeof m, "%s (os error %d)", m0, errno);
  size_t n = strlen(m);
  char* t = malloc(n + 1);
  t[0] = '0';
  memcpy(t + 1, m, n);
  Term r = io_str(e, t, n + 1);
  free(t);
  return r;
}

Term s_fx_run(Env e, Term* f, IoWork* w) {
  uint64_t n0 = 0, n1 = 0, n2 = 0;
  char* op = io_cstr(e, f[0], &n0);
  char* a = io_cstr(e, f[1], &n1);
  char* b = io_cstr(e, f[2], &n2);
  Term r;
  fflush(stdout);
  if (strcmp(op, "r") == 0) {
    FILE* fp = fopen(a, "rb");
    if (!fp) { r = s_no(e, strerror(errno)); }
    else {
      size_t cap = 1 << 16, len = 0, k;
      char* buf = malloc(cap);
      while ((k = fread(buf + len, 1, cap - len, fp)) > 0) { len += k; if (len == cap) { cap *= 2; buf = realloc(buf, cap); } }
      fclose(fp);
      r = s_ok(e, buf, len);
      free(buf);
    }
  } else if (strcmp(op, "fo") == 0) {
    char* sep = strchr(b, 1);
    const char* mode = sep ? sep + 1 : "r";
    if (sep) *sep = 0;
    char m[8]; int k = 0;
    for (const char* c = mode; *c && k < 7; c++) if (*c != 'b') m[k++] = *c;
    m[k] = 0;
    const char* cm = strcmp(m, "r") == 0 ? "rb" : strcmp(m, "w") == 0 ? "wb" : strcmp(m, "a") == 0 ? "ab"
      : strcmp(m, "r+") == 0 ? "r+b" : strcmp(m, "w+") == 0 ? "w+b" : strcmp(m, "a+") == 0 ? "a+b" : NULL;
    if (!cm) {
      char t[64]; snprintf(t, 64, "0bad mode '%s'", mode); r = io_str(e, t, strlen(t));
    } else {
      FILE* fp = fopen(a, cm);
      if (!fp) r = s_no(e, strerror(errno));
      else {
        s_drop(b);
        int i = 0; while (i < S_FILES && s_files[i].name) i++;
        if (i == S_FILES) { fclose(fp); r = io_str(e, "0too many open files", 20); }
        else { s_files[i].name = strdup(b); s_files[i].fp = fp; s_files[i].eof = 0; r = s_ok(e, "", 0); }
      }
    }
  } else if (strcmp(op, "mv") == 0) {
    if (rename(a, b) != 0) r = s_no(e, strerror(errno)); else r = s_ok(e, "", 0);
  } else if (strcmp(op, "fc") == 0) {
    s_drop(a); r = s_ok(e, "", 0);
  } else if (strcmp(op, "feof") == 0) {
    int i = s_find(a); r = s_ok(e, i < 0 || s_files[i].eof ? "1" : "0", 1);
  } else if (op[0] == 'f' && op[1] != 0) {
    int i = s_find(a);
    if (i < 0) { r = io_str(e, "0no open file", 13); }
    else if (strcmp(op, "fw") == 0) {
      if (fwrite(b, 1, n2, s_files[i].fp) != n2) r = s_no(e, strerror(errno)); else { fflush(s_files[i].fp); r = s_ok(e, "", 0); }
    } else if (strcmp(op, "fat") == 0) {
      char* sep = strchr(b, 1);
      double off = atof(b);
      const char* data = sep ? sep + 1 : "";
      size_t dn = sep ? n2 - (size_t)(sep + 1 - b) : 0;
      if (fseek(s_files[i].fp, (long)off, SEEK_SET) != 0 || fwrite(data, 1, dn, s_files[i].fp) != dn) r = s_no(e, strerror(errno));
      else { fflush(s_files[i].fp); r = s_ok(e, "", 0); }
    } else if (strcmp(op, "fra") == 0) {
      double off = 0, len = 0; sscanf(b, "%lf %lf", &off, &len);
      size_t n = len > 0 ? (size_t)len : 0, got = 0;
      char* buf = malloc(n + 1);
      if (fseek(s_files[i].fp, (long)off, SEEK_SET) == 0) got = fread(buf, 1, n, s_files[i].fp);
      s_files[i].eof = got < n;
      r = s_ok(e, buf, got); free(buf);
    } else if (strcmp(op, "frl") == 0) {
      char* line = NULL; size_t cap = 0;
      ssize_t n = getline(&line, &cap, s_files[i].fp);
      if (n < 0) { s_files[i].eof = 1; r = s_ok(e, "", 0); }
      else {
        s_files[i].eof = 0;
        if (n > 0 && line[n - 1] == '\n') n--;
        if (n > 0 && line[n - 1] == '\r') n--;
        r = s_ok(e, line, n);
      }
      free(line);
    } else r = io_str(e, "0unknown effect", 15);
  } else if (strcmp(op, "ra") == 0) {
    double off = 0, len = 0;
    sscanf(b, "%lf %lf", &off, &len);
    FILE* fp = fopen(a, "rb");
    if (!fp) { r = s_no(e, strerror(errno)); }
    else {
      size_t n = len > 0 ? (size_t)len : 0;
      char* buf = malloc(n + 1);
      size_t got = 0;
      if (fseek(fp, (long)off, SEEK_SET) == 0) got = fread(buf, 1, n, fp);
      fclose(fp);
      r = s_ok(e, buf, got);
      free(buf);
    }
  } else if (strcmp(op, "w") == 0 || strcmp(op, "a") == 0) {
    FILE* fp = fopen(a, op[0] == 'w' ? "wb" : "ab");
    if (!fp) { r = s_no(e, strerror(errno)); }
    else { fwrite(b, 1, n2, fp); fclose(fp); r = s_ok(e, "", 0); }
  } else if (strcmp(op, "sh") == 0) {
    int p[2];
    if (pipe(p) != 0) { r = s_no(e, strerror(errno)); }
    else {
      pid_t pid = fork();
      if (pid == 0) {
        int nul = open("/dev/null", O_RDONLY);
        dup2(nul, 0); dup2(p[1], 1); close(p[0]); close(p[1]);
        execl("/bin/sh", "sh", "-c", a, (char*)NULL);
        _exit(127);
      }
      close(p[1]);
      size_t cap = 1 << 16, len = 0; ssize_t k;
      char* buf = malloc(cap + 32);
      int hdr = 0;
      while ((k = read(p[0], buf + 32 + len, cap - len)) > 0) { len += k; if (len == cap) { cap *= 2; buf = realloc(buf, cap + 32); } }
      close(p[0]);
      int st = 0; waitpid(pid, &st, 0);
      int rc = WIFEXITED(st) ? WEXITSTATUS(st) : -1;
      char head[32]; hdr = snprintf(head, 32, "%d\001", rc);
      memcpy(buf + 32 - hdr, head, hdr);
      r = s_ok(e, buf + 32 - hdr, len + hdr);
      free(buf);
    }
  } else if (strcmp(op, "chk") == 0) {
    FILE* fp = fopen(a, "rb");
    if (!fp) { r = s_no(e, strerror(errno)); }
    else {
      uint64_t h = 0xcbf29ce484222325ull; int c;
      while ((c = fgetc(fp)) != EOF) { h ^= (uint64_t)(unsigned char)c; h *= 0x100000001b3ull; }
      fclose(fp);
      char t[32]; snprintf(t, 32, "%016llx", (unsigned long long)h);
      if (strcmp(t, b) == 0) r = s_ok(e, "", 0);
      else { const char* m = "0changed since this program was built; build it again"; r = io_str(e, m, strlen(m)); }
    }
  } else if (strcmp(op, "pn") == 0) {
    s_seed_from(b);
    double x = (double)(s_next() >> 11) / 9007199254740992.0;
    char t[64]; int k = snprintf(t, 64, "%.17g", x);
    r = s_ok(e, t, k);
  } else if (strcmp(op, "ps") == 0) {
    static const char* az = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    s_seed_from(b);
    char t[32];
    for (int i = 0; i < 32; i++) t[i] = az[s_next() % 52];
    r = s_ok(e, t, 32);
  } else {
    r = s_no(e, "unknown effect");
  }
  free(op); free(a); free(b);
  return r;
}

static void __attribute__((constructor)) s_fx_use(void) {
  io_eff(CID(S.fx), s_fx_run);
}
