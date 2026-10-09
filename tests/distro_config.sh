#!/usr/bin/env bash
# Distro policy is data, not code: resources/backend/distro.conf (overridden by /etc/voyage/distro.conf in the
# live image) and hook directories. Hooks run as root, so they are held to strict rules.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIB="$HERE/resources/backend/distro_config.sh"
[ -f "$LIB" ] || { echo "FAIL $LIB does not exist"; exit 1; }

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }
ok()   { echo "ok   $1"; }
bad()  { echo "FAIL $1"; fails=$((fails + 1)); }

HOOKS_TRUSTED_UID="$(id -u)"
DISTRO_CONF_DEFAULT="$T/default.conf"
DISTRO_CONF_OVERRIDE="$T/override.conf"
. "$LIB"

cat > "$DISTRO_CONF_DEFAULT" <<'C'
# comment line
default-shell /usr/bin/fish fish-shell   # trailing comment
remove voyage xmirror
remove dialog
install nano htop
enable sshd
disable dhcpcd
C

# ---- parsing
check "values of a key, across lines, comments ignored" "$(distro_values remove | tr '\n' ' ')" "voyage xmirror dialog "
check "a single value"                    "$(distro_value default-shell)" "/usr/bin/fish"
check "the optional package of the shell" "$(distro_values default-shell | sed -n 2p)" "fish-shell"
check "an absent key has no values"       "$(distro_values nothing | wc -l)" "0"
check "install list"                      "$(distro_values install | tr '\n' ' ')" "nano htop "
distro_conf_check 2>/dev/null && ok "a good file passes the check" || bad "a good file was rejected"

# ---- override replaces the shipped file entirely
printf 'default-shell none\nremove foo\n' > "$DISTRO_CONF_OVERRIDE"
check "the override wins"                 "$(distro_value default-shell)" "none"
check "and replaces, not merges"          "$(distro_values remove | tr '\n' ' ')" "foo "
check "nothing left from the shipped file" "$(distro_values install | wc -l)" "0"
rm -f "$DISTRO_CONF_OVERRIDE"

# ---- validation: a typo must stop the install before anything is touched
try() { printf '%s\n' "$2" > "$DISTRO_CONF_DEFAULT"; if distro_conf_check >/dev/null 2>"$T/err"; then bad "$1: accepted"; else ok "$1"; fi; }
try "unknown key"                       "instal nano"
try "package name with a shell metacharacter" 'install nano;rm'
try "package name starting with a dash" 'install -Rf'
try "service name with a slash"         'enable ../etc'
try "shell that is not an absolute path" 'default-shell fish'
try "shell with a space-less odd char"  'default-shell /usr/bin/fi$h'
try "shell path with .. segments"       'default-shell /usr/bin/../../tmp/x'
try "shell path with a double slash"    'default-shell /usr//bin/fish'
try "key without values"                'install'
try "too many words for the shell"      'default-shell /usr/bin/fish fish-shell extra'
printf 'default-shell /usr/bin/fish\nbogus x\n' > "$DISTRO_CONF_DEFAULT"; distro_conf_check 2>"$T/err"
grep -q ':2:' "$T/err" && ok "the message names the line" || bad "message lacks the line number: $(cat "$T/err")"

# ---- the override file is held to the same trust rules as hooks
printf 'default-shell none\n' > "$DISTRO_CONF_OVERRIDE"
distro_conf_check 2>/dev/null && ok "an override owned by the trusted user passes" || bad "trusted override rejected"
chmod 666 "$DISTRO_CONF_OVERRIDE"
distro_conf_check 2>"$T/err" && bad "a world-writable override was accepted" || ok "a world-writable override is refused"
grep -q "override" "$T/err" && ok "and the message says why" || bad "no explanation: $(cat "$T/err")"
rm -f "$DISTRO_CONF_OVERRIDE"
printf 'default-shell none\n' > "$DISTRO_CONF_OVERRIDE"
distro_conf_check 2>"$T/err" && ok "an override without remove still passes" || bad "override without remove rejected"
grep -q "remove" "$T/err" && ok "but it says the installer stays on the new system" || bad "no note about the missing remove line: $(cat "$T/err")"
printf 'remove voyage\n' > "$DISTRO_CONF_OVERRIDE"
distro_conf_check 2>"$T/err"; check "no note when there is a remove line" "$(wc -c < "$T/err")" "0"
rm -f "$DISTRO_CONF_OVERRIDE"
printf 'install nano\n' > "$DISTRO_CONF_DEFAULT"

# ---- the shipped default keeps today's behaviour
DISTRO_CONF_DEFAULT="$HERE/resources/backend/distro.conf"
distro_conf_check 2>"$T/err" && ok "the shipped distro.conf is valid" || bad "shipped distro.conf: $(cat "$T/err")"
check "default login shell stays fish"  "$(distro_value default-shell)" "/usr/bin/fish"
check "installer-only packages are unchanged" "$(distro_values remove | tr '\n' ' ')" "voyage xmirror dialog xtools-minimal "

# ---- hooks
H="$T/hooks"; OUT="$T/out"; export OUT; mkdir -p "$H/post-copy" "$T/etc-hooks/post-copy"
HOOKS_DIRS="$H:$T/etc-hooks"; HOOKS_TRUSTED_UID="$(id -u)"
TARGETDIR=/mnt/target; INIT_SYSTEM=dinit; CONF_FILE=/tmp/conf
mk() { printf '#!/bin/sh\n%s\n' "$2" > "$1"; chmod "${3:-755}" "$1"; }
mk "$H/post-copy/20-second" 'echo "second $VOYAGE_STAGE $TARGETDIR $INIT_SYSTEM" >> "$OUT"'
mk "$H/post-copy/10-first"  'echo first >> "$OUT"'
mk "$H/post-copy/30-skipped.disabled" 'echo SHOULD-NOT-RUN >> "$OUT"'
: > "$OUT"
run_hooks post-copy >/dev/null 2>&1 && ok "hooks run" || bad "hooks failed"
check "in name order, with the stage environment" "$(tr '\n' '|' < "$OUT")" "first|second post-copy /mnt/target dinit|"
check "a .disabled file is skipped" "$(grep -c SHOULD-NOT-RUN "$OUT")" "0"

mk "$T/etc-hooks/post-copy/10-first" 'echo override >> "$OUT"'
: > "$OUT"; run_hooks post-copy >/dev/null 2>&1
check "a hook in /etc replaces the shipped one of the same name" "$(tr '\n' '|' < "$OUT")" "override|second post-copy /mnt/target dinit|"
rm -f "$T/etc-hooks/post-copy/10-first"

# an admin can switch a shipped hook off from /etc
mk "$T/etc-hooks/post-copy/10-first.disabled" 'exit 9' 644
rm -f "$T/etc-hooks/post-copy/10-first"
: > "$OUT"; run_hooks post-copy >/dev/null 2>&1
check "a .disabled file in /etc switches off the shipped hook of that name" "$(tr '\n' '|' < "$OUT")" "second post-copy /mnt/target dinit|"
rm -f "$T/etc-hooks/post-copy/10-first.disabled"
mk "$T/etc-hooks/post-copy/20-second" 'echo SHOULD-NOT-RUN >> "$OUT"' 644
: > "$OUT"; run_hooks post-copy >/dev/null 2>&1
check "so does a non-executable file of the same name" "$(tr '\n' '|' < "$OUT")" "first|"
rm -f "$T/etc-hooks/post-copy/20-second"

# a hooks directory whose path contains a space still works
mkdir -p "$T/with space/post-copy"; mk "$T/with space/post-copy/10-spaced" 'echo spaced >> "$OUT"'
OLD_DIRS="$HOOKS_DIRS"; HOOKS_DIRS="$T/with space"; : > "$OUT"; run_hooks post-copy >/dev/null 2>&1
check "a directory with a space in its path is not skipped" "$(tr '\n' '|' < "$OUT")" "spaced|"
HOOKS_DIRS="$OLD_DIRS"

# every parent directory counts: a hook below a directory others can write to could be swapped under us
mkdir -p "$T/wparent/hooks/post-copy"; mk "$T/wparent/hooks/post-copy/10-x" 'echo CHAIN >> "$OUT"'
chmod 777 "$T/wparent"
OLD_DIRS="$HOOKS_DIRS"; HOOKS_DIRS="$T/wparent/hooks"; : > "$OUT"
run_hooks post-copy >/dev/null 2>"$T/err" && bad "a hook below a world-writable parent ran" || ok "a world-writable parent directory is refused"
check "and it did not run" "$(grep -c CHAIN "$OUT")" "0"
chmod 1777 "$T/wparent"; : > "$OUT"
run_hooks post-copy >/dev/null 2>&1 && ok "a sticky shared parent (like /tmp) is fine for an entry we own" || bad "sticky parent was refused"
chmod 755 "$T/wparent"
mkdir -p "$T/lnk-real/post-copy"; mk "$T/lnk-real/post-copy/10-y" 'echo LINK >> "$OUT"'; ln -s "$T/lnk-real" "$T/lnk"
HOOKS_DIRS="$T/lnk"; : > "$OUT"; run_hooks post-copy >/dev/null 2>&1 && ok "a symlinked hooks directory is followed and checked" || bad "symlinked dir refused"
chmod 777 "$T/lnk-real"; run_hooks post-copy >/dev/null 2>&1 && bad "a writable real directory behind a symlink ran" || ok "and its real location is what is judged"
chmod 755 "$T/lnk-real"
HOOKS_DIRS="$OLD_DIRS"

# the override has parents too
mkdir -p "$T/ov"; printf 'default-shell none\n' > "$T/ov/distro.conf"; chmod 777 "$T/ov"
DISTRO_CONF_OVERRIDE="$T/ov/distro.conf"
distro_conf_check 2>/dev/null && bad "an override in a world-writable directory was accepted" || ok "an override in a world-writable directory is refused"
chmod 755 "$T/ov"; DISTRO_CONF_OVERRIDE="$T/override.conf"

run_hooks no-such-stage >/dev/null 2>&1 && ok "a stage without a directory is fine" || bad "missing stage dir failed"
run_hooks "../etc" >/dev/null 2>&1 && bad "a stage with a path was accepted" || ok "a stage name cannot be a path"

mk "$H/post-copy/40-fails" 'exit 3'
run_hooks post-copy >/dev/null 2>"$T/err" && bad "a failing hook was ignored" || ok "a failing hook is fatal"
grep -q "40-fails" "$T/err" && ok "and it is named" || bad "failing hook not named"
mv "$H/post-copy/40-fails" "$H/post-copy/40-fails.optional"
run_hooks post-copy >/dev/null 2>&1 && ok "a failing .optional hook is tolerated" || bad ".optional hook was fatal"
rm -f "$H/post-copy/40-fails.optional"

mk "$H/post-copy/50-writable" 'echo EVIL >> "$OUT"' 777
: > "$OUT"; run_hooks post-copy >/dev/null 2>"$T/err" && bad "a world-writable hook ran" || ok "a world-writable hook is refused"
check "and it did not run" "$(grep -c EVIL "$OUT")" "0"
rm -f "$H/post-copy/50-writable"

mk "$H/post-copy/60-foreign" 'echo EVIL2 >> "$OUT"' 755
HOOKS_TRUSTED_UID=$(( $(id -u) + 1 ))
: > "$OUT"; run_hooks post-copy >/dev/null 2>&1 && bad "a hook owned by someone else ran" || ok "a hook not owned by the trusted user is refused"
check "and nothing ran" "$(wc -c < "$OUT")" "0"

[ "$fails" -eq 0 ]
