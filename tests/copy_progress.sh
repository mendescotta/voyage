#!/usr/bin/env bash
# The root copy reports real progress: tar's checkpoint records become `>>> PROGRESS n` lines on fd 3
# (n is a percentage of the bytes `du` counted), everything else tar says still goes to the log.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$HERE/resources/backend/backend_install.sh"

eval "$(sed -n '/^copy_progress_pct()/,/^}/p;/^copy_progress_reader()/,/^}/p' "$SCRIPT")"
type copy_progress_pct >/dev/null 2>&1 || { echo "FAIL copy_progress_pct is not defined in the backend"; exit 1; }
type copy_progress_reader >/dev/null 2>&1 || { echo "FAIL copy_progress_reader is not defined in the backend"; exit 1; }

fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }

# 10240 bytes per tar record: 1000 records of a 20480000-byte tree is half
check "half way"                    "$(copy_progress_pct 1000 20480000)" "50"
check "start"                       "$(copy_progress_pct 0 20480000)" "0"
check "never reports 100 before the end (headers add a little)" "$(copy_progress_pct 2100 20480000)" "99"
check "far past the total is capped" "$(copy_progress_pct 999999 20480000)" "99"
check "unknown total gives nothing" "$(copy_progress_pct 100 0)" ""
check "empty total gives nothing"   "$(copy_progress_pct 100 '')" ""
check "garbage records give nothing" "$(copy_progress_pct abc 20480000)" ""
check "garbage total gives nothing"  "$(copy_progress_pct 100 12x)" ""

out="$(mktemp)"; err="$(mktemp)"; trap 'rm -f "$out" "$err"' EXIT
printf '%s\n' 'tar: @CKPT 500' 'tar: @CKPT 600' 'tar: @CKPT 1000' 'tar: Removing leading `/'"'"' from member names' 'tar: @CKPT 1000' \
  | copy_progress_reader 20480000 3>"$out" 2>"$err"
check "one line per change, none for repeats" "$(tr '\n' '|' <"$out")" ">>> PROGRESS 25|>>> PROGRESS 30|>>> PROGRESS 50|"
check "other tar messages still reach the log" "$(cat "$err")" "tar: Removing leading \`/' from member names"

: >"$out"; : >"$err"
printf '%s\n' 'tar: @CKPT 500' 'tar: some warning' | copy_progress_reader 0 3>"$out" 2>"$err"
check "no total: no progress lines"  "$(cat "$out")" ""
check "no total: warnings still logged" "$(cat "$err")" "tar: some warning"

[ "$fails" -eq 0 ]
