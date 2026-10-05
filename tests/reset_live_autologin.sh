#!/usr/bin/env bash
# copy_rootfs tars the running live system, so the installed system inherits the
# live user's display-manager autologin. lightdm reads lightdm.conf AFTER
# lightdm.conf.d, so a leftover live value would override the installer's choice.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$HERE/resources/backend/backend_install.sh"

eval "$(sed -n '/^reset_live_autologin()/,/^}/p' "$SCRIPT")"
type reset_live_autologin >/dev/null 2>&1 || { echo "FAIL reset_live_autologin is not defined in the backend"; exit 1; }

W="$(mktemp -d)"; trap 'rm -rf "$W"' EXIT
TARGETDIR="$W/target"
mkdir -p "$TARGETDIR/etc/lightdm/lightdm.conf.d" "$TARGETDIR/etc/gdm" "$TARGETDIR/usr/share/xsessions"

# What the live hook leaves behind (vmklive display-manager-autologin.sh).
cat > "$TARGETDIR/etc/lightdm/lightdm.conf" <<'CONF'
[LightDM]
[Seat:*]
autologin-user=anon
autologin-user-timeout=0
autologin-session=xfce
user-session=xfce
CONF
printf '[daemon]\nAutomaticLoginEnable=true\nAutomaticLogin=anon\n' > "$TARGETDIR/etc/gdm/custom.conf"
printf '[Autologin]\nUser=anon\nSession=plasma.desktop\n' > "$TARGETDIR/etc/sddm.conf"

fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }

reset_live_autologin anon

check "lightdm: live autologin-user is gone" "$(grep -c '^autologin-user=' "$TARGETDIR/etc/lightdm/lightdm.conf")" "0"
check "lightdm: live timeout is gone" "$(grep -c '^autologin-user-timeout=' "$TARGETDIR/etc/lightdm/lightdm.conf")" "0"
check "lightdm: session settings are kept" "$(grep -c '^autologin-session=xfce' "$TARGETDIR/etc/lightdm/lightdm.conf")" "1"
check "gdm: live autologin is switched off" "$(grep -E '^AutomaticLogin(Enable)?=' "$TARGETDIR/etc/gdm/custom.conf" | tr '\n' ' ')" "AutomaticLoginEnable=false "
check "sddm: live [Autologin] file is removed" "$([ -e "$TARGETDIR/etc/sddm.conf" ] && echo present || echo gone)" "gone"

# A different user's autologin is left alone.
printf '[Seat:*]\nautologin-user=someone\n' > "$TARGETDIR/etc/lightdm/lightdm.conf"
reset_live_autologin anon
check "lightdm: another user's autologin is untouched" "$(grep -c '^autologin-user=someone' "$TARGETDIR/etc/lightdm/lightdm.conf")" "1"

# End to end with lightdm's own resolver: the installer's drop-in must win.
if command -v lightdm >/dev/null 2>&1; then
	cat > "$W/main.conf" <<'CONF'
[Seat:*]
autologin-user=anon
autologin-user-timeout=0
autologin-session=xfce
CONF
	mkdir -p "$W/data/lightdm/lightdm.conf.d"
	printf '[Seat:*]\nautologin-user=gui\nautologin-user-timeout=0\n' > "$W/data/lightdm/lightdm.conf.d/50-voyage-autologin.conf"
	TARGETDIR="$W/t2"; mkdir -p "$TARGETDIR/etc/lightdm"; cp "$W/main.conf" "$TARGETDIR/etc/lightdm/lightdm.conf"
	reset_live_autologin anon
	got="$(XDG_DATA_DIRS="$W/data:/usr/share" lightdm --show-config -c "$TARGETDIR/etc/lightdm/lightdm.conf" 2>&1 | grep -E '^[A-Z]  autologin-user=' | sed 's/^[A-Z]  //')"
	check "lightdm resolves autologin-user to the installer's user" "$got" "autologin-user=gui"
fi

[ "$fails" -eq 0 ]
