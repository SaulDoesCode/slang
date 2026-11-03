#!/bin/sh
# check.sh [names…]: each program here built with s3 native and run on its .in and .args, against the machine
S3=${S3:-$(dirname "$0")/../../../target/release/s3}
cd "$(dirname "$0")"
out=${TMPDIR:-/tmp}/s3-native-check
mkdir -p "$out"
names=${*:-$(ls *.s | sed 's/\.s$//')}
pass=0; fail=0
for t in $names; do
  a=$(cat "$t.args" 2>/dev/null)
  in=/dev/null; [ -f "$t.in" ] && in="$t.in"
  if ! "$S3" native "$t.s" -o "$out/$t" > "$out/$t.log" 2>&1; then
    echo "BUILD $t"; tail -3 "$out/$t.log"; fail=$((fail+1)); continue
  fi
  "$S3" run "$t.s" $a < "$in" > "$out/$t.m" 2>&1; echo "rc $?" >> "$out/$t.m"
  "$out/$t" $a < "$in" > "$out/$t.b" 2>&1; echo "rc $?" >> "$out/$t.b"
  if cmp -s "$out/$t.m" "$out/$t.b"; then echo "ok $t"; pass=$((pass+1)); else echo "DIFF $t"; diff "$out/$t.m" "$out/$t.b" | head -8; fail=$((fail+1)); fi
done
echo "$pass same, $fail not"
