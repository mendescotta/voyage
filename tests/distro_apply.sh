#!/usr/bin/env bash
# The backend applies distro.conf: login shell, extra packages, services; and the hook stages sit at the
# right places of the install flow.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$HERE/resources/backend/backend_install.sh"

eval "$(sed -n '/^enable_service()/,/^}/p;/^disable_service()/,/^}/p;/^enable_service_checked()/,/^}/p;/^apply_distro_services()/,/^}/p;/^install_distro_packages()/,/^}/p;/^apply_login_shell()/,/^}/p;/^set_login_shells()/,/^}/p;/^install_vbox_guest()/,/^}/p' "$SCRIPT")"
for f in enable_service disable_service enable_service_checked apply_distro_services install_distro_packages apply_login_shell set_login_shells install_vbox_guest; do
	type "$f" >/dev/null 2>&1 || { echo "FAIL $f is not defined in the backend"; exit 1; }
done

T="$(mktemp -d)"; trap 'rm -rf "$T"' EXIT
fails=0
check() { if [ "$2" = "$3" ]; then echo "ok   $1"; else echo "FAIL $1: got '$2', want '$3'"; fails=$((fails + 1)); fi; }

DISTRO_CONF_DEFAULT="$T/distro.conf"; DISTRO_CONF_OVERRIDE=/nonexistent
. "$HERE/resources/backend/distro_config.sh"
TARGETDIR="$T/target"; CALLS=""; INSTALLED=" "; OPTS_UPDATE=1; OPTS_USERLOGIN=gui
mkdir -p "$TARGETDIR/etc" "$TARGETDIR/usr/bin" "$TARGETDIR/bin"
for s in usr/bin/fish bin/bash; do printf '#!/bin/sh\n' > "$TARGETDIR/$s"; chmod +x "$TARGETDIR/$s"; done
log_ui() { LOGS="$LOGS|$*"; }; LOGS=""
OPTS_ROOTSHELL=""; OPTS_USERSHELL=""; OPTS_VBOX=0
get_option() { case "$1" in UPDATE) echo "$OPTS_UPDATE" ;; USERLOGIN) echo "$OPTS_USERLOGIN" ;; ROOTSHELL) echo "$OPTS_ROOTSHELL" ;; USERSHELL) echo "$OPTS_USERSHELL" ;; VBOXGUEST) echo "$OPTS_VBOX" ;; esac; }
chroot() {
	shift
	case "$1" in
		xbps-query) case "$INSTALLED" in *" $2 "*) return 0 ;; *) return 1 ;; esac ;;
		xbps-install) CALLS="$CALLS|$*"; [ "${OFFLINE:-}" = 1 ] && return 1; return 0 ;;
		usermod) CALLS="$CALLS|$*" ;;
	esac
}

# ---- login shells: one choice for the user and one for root
DISTRO_CONF_DEFAULT="$HERE/resources/backend/distro.conf"
mkdir -p "$TARGETDIR/bin" "$TARGETDIR/usr/bin"
for sh in /bin/bash /usr/bin/zsh /usr/bin/fish; do printf '#!/bin/sh\n' > "$TARGETDIR$sh"; chmod +x "$TARGETDIR$sh"; done
ln -s dash "$TARGETDIR/bin/sh"
reset() { CALLS=""; LOGS=""; INSTALLED=" "; OFFLINE=0; OPTS_ROOTSHELL=""; OPTS_USERSHELL=""; OPTS_UPDATE=1; rm -f "$TARGETDIR/etc/shells"; }

reset; set_login_shells
check "no choice: both accounts get the default, bash" "$CALLS" "|usermod -s /bin/bash root|usermod -s /bin/bash gui"
check "bash needs no installation" "$(printf '%s' "$CALLS" | grep -c xbps-install)" "0"
check "the shell is listed in /etc/shells" "$(grep -c '^/bin/bash$' "$TARGETDIR/etc/shells")" "1"
check "/bin/sh is still dash" "$(readlink "$TARGETDIR/bin/sh")" "dash"

reset; OPTS_ROOTSHELL=fish; OPTS_USERSHELL=zsh; set_login_shells
check "root fish, user zsh: both packages installed, each set for its account" "$CALLS" "|xbps-install -Sy fish-shell|usermod -s /usr/bin/fish root|xbps-install -Sy zsh|usermod -s /usr/bin/zsh gui"
check "both are in /etc/shells" "$(grep -cE '^/usr/bin/(fish|zsh)$' "$TARGETDIR/etc/shells")" "2"
check "/bin/sh is still dash after a choice" "$(readlink "$TARGETDIR/bin/sh")" "dash"

reset; INSTALLED=" zsh "; OPTS_USERSHELL=zsh; set_login_shells
check "an installed shell is not installed again" "$CALLS" "|usermod -s /bin/bash root|usermod -s /usr/bin/zsh gui"

reset; OPTS_USERSHELL=zsh; OFFLINE=1; mv "$TARGETDIR/usr/bin/zsh" "$T/zsh.hold"
set_login_shells
check "zsh cannot be installed and is absent: that account falls back to bash" "$CALLS" "|usermod -s /bin/bash root|xbps-install -Sy zsh|usermod -s /bin/bash gui"
check "and the install says so" "$(printf '%s' "$LOGS" | grep -c 'zsh')" "1"
mv "$T/zsh.hold" "$TARGETDIR/usr/bin/zsh"

reset; OPTS_USERSHELL=ksh; set_login_shells
check "a shell the config does not define falls back to the default" "$CALLS" "|usermod -s /bin/bash root|usermod -s /bin/bash gui"
check "and is reported" "$(printf '%s' "$LOGS" | grep -c 'ksh')" "1"

reset; INSTALLED=" fish-shell "; OPTS_ROOTSHELL=fish; mv "$TARGETDIR/usr/bin/fish" "$T/fish.hold"
set_login_shells
check "a package that left no binary behind is not trusted" "$CALLS" "|usermod -s /bin/bash root|usermod -s /bin/bash gui"
mv "$T/fish.hold" "$TARGETDIR/usr/bin/fish"

# a distro whose default is zsh
printf 'shell bash /bin/bash\nshell zsh /usr/bin/zsh zsh\ndefault-shell zsh\n' > "$T/distro.conf"; DISTRO_CONF_DEFAULT="$T/distro.conf"
reset; INSTALLED=" zsh "; set_login_shells
check "the default comes from distro.conf" "$CALLS" "|usermod -s /usr/bin/zsh root|usermod -s /usr/bin/zsh gui"

# a failing usermod is reported, not swallowed
DISTRO_CONF_DEFAULT="$HERE/resources/backend/distro.conf"; reset
chroot() { shift; case "$1" in xbps-query) return 0 ;; usermod) return 1 ;; esac; }
set_login_shells
check "usermod failures are logged for root and the user" "$(printf '%s' "$LOGS" | grep -o 'could not set' | wc -l)" "2"
chroot() {
	shift
	case "$1" in
		xbps-query) case "$INSTALLED" in *" $2 "*) return 0 ;; *) return 1 ;; esac ;;
		xbps-install) CALLS="$CALLS|$*"; [ "${OFFLINE:-}" = 1 ] && return 1; return 0 ;;
		usermod) CALLS="$CALLS|$*" ;;
	esac
}
reset; DISTRO_CONF_DEFAULT="$T/distro.conf"

# ---- extra packages
printf 'install nano htop\n' > "$T/distro.conf"; CALLS=""
install_distro_packages
check "extra packages are installed in one call" "$CALLS" "|xbps-install -Sy nano htop"
CALLS=""; OPTS_UPDATE=0; install_distro_packages
check "offline installs skip them" "$CALLS" ""
OPTS_UPDATE=1; printf '# nothing\n' > "$T/distro.conf"; CALLS=""; install_distro_packages
check "no packages: nothing runs" "$CALLS" ""

# ---- services, on both inits
printf 'enable sshd\nenable ghost\ndisable dhcpcd\n' > "$T/distro.conf"
INIT_SYSTEM=dinit; rm -rf "$TARGETDIR/etc"; mkdir -p "$TARGETDIR/etc/dinit.d/boot.d"
touch "$TARGETDIR/etc/dinit.d/sshd" "$TARGETDIR/etc/dinit.d/dhcpcd"; ln -s /etc/dinit.d/dhcpcd "$TARGETDIR/etc/dinit.d/boot.d/dhcpcd"
apply_distro_services 2>"$T/err"
check "dinit: a service is enabled"   "$(readlink "$TARGETDIR/etc/dinit.d/boot.d/sshd")" "/etc/dinit.d/sshd"
check "dinit: a service is disabled"  "$([ -e "$TARGETDIR/etc/dinit.d/boot.d/dhcpcd" ] || [ -L "$TARGETDIR/etc/dinit.d/boot.d/dhcpcd" ] && echo present || echo gone)" "gone"
check "dinit: a missing service is skipped with a warning" "$(grep -c 'no service ghost' "$T/err")" "1"
check "dinit: and no dangling link is made" "$([ -L "$TARGETDIR/etc/dinit.d/boot.d/ghost" ] && echo link || echo none)" "none"

INIT_SYSTEM=runit; rm -rf "$TARGETDIR/etc"; mkdir -p "$TARGETDIR/etc/sv/sshd" "$TARGETDIR/etc/runit/runsvdir/default"
apply_distro_services 2>/dev/null
check "runit: a service is enabled"   "$(readlink "$TARGETDIR/etc/runit/runsvdir/default/sshd")" "/etc/sv/sshd"

# ---- VirtualBox guest additions
DISTRO_CONF_DEFAULT="$HERE/resources/backend/distro.conf"; INIT_SYSTEM=dinit
rm -rf "$TARGETDIR/etc"; mkdir -p "$TARGETDIR/etc/dinit.d/boot.d"; touch "$TARGETDIR/etc/dinit.d/vboxservice"
reset; OPTS_VBOX=0; install_vbox_guest
check "not requested: nothing happens" "$CALLS" ""
check "and no service is started" "$([ -L "$TARGETDIR/etc/dinit.d/boot.d/vboxservice" ] && echo on || echo off)" "off"

reset; OPTS_VBOX=1; install_vbox_guest
check "requested, online: the guest package is installed" "$CALLS" "|xbps-install -Sy virtualbox-ose-guest"
check "and vboxservice is enabled" "$(readlink "$TARGETDIR/etc/dinit.d/boot.d/vboxservice")" "/etc/dinit.d/vboxservice"

rm -f "$TARGETDIR/etc/dinit.d/boot.d/vboxservice"
reset; OPTS_VBOX=1; OPTS_UPDATE=0; install_vbox_guest
check "requested, offline, not on the image: no install attempt" "$CALLS" ""
check "and the user is told" "$(printf '%s' "$LOGS" | grep -c 'internet')" "1"
check "and nothing is enabled" "$([ -L "$TARGETDIR/etc/dinit.d/boot.d/vboxservice" ] && echo on || echo off)" "off"

reset; OPTS_VBOX=1; OPTS_UPDATE=0; INSTALLED=" virtualbox-ose-guest "; install_vbox_guest
check "offline but already on the image: no install, service enabled" "$CALLS|$(readlink "$TARGETDIR/etc/dinit.d/boot.d/vboxservice")" "|/etc/dinit.d/vboxservice"

rm -f "$TARGETDIR/etc/dinit.d/boot.d/vboxservice"
reset; OPTS_VBOX=1; OFFLINE=1; install_vbox_guest
check "a failed install is reported and does not stop the installer" "$(printf '%s' "$LOGS" | grep -c 'Could not install')" "1"
check "and nothing is enabled" "$([ -L "$TARGETDIR/etc/dinit.d/boot.d/vboxservice" ] && echo on || echo off)" "off"

# ---- contract: what the GUI writes into the conf file is what the backend reads
WRITER="$HERE/src/backend/install_runner.rs"
for key in USERSHELL ROOTSHELL VBOXGUEST; do
	w=$(grep -c "\"$key\"," "$WRITER" 2>/dev/null); r=$(grep -c "get_option $key" "$SCRIPT")
	check "$key is written by the GUI and read by the backend" "$([ "$w" -ge 1 ] && [ "$r" -ge 1 ] && echo both || echo "writer=$w reader=$r")" "both"
done

# ---- the stages sit where the flow needs them
line() { grep -n "$1" "$SCRIPT" | tail -1 | cut -d: -f1; }
order_ok() { [ -n "$1" ] && [ -n "$2" ] && [ "$1" -lt "$2" ]; }
conf=$(line '^distro_conf_check ||'); fs=$(line '^create_filesystems$'); pre=$(line 'run_hooks pre-copy'); cp_=$(line '^copy_rootfs$')
post=$(line 'run_hooks post-copy'); dracut=$(line 'chroot "\$TARGETDIR" dracut'); rm_=$(line '^remove_installer_packages$'); fin=$(line 'run_hooks post-install'); umount_=$(line '^umount_filesystems$')
order_ok "$conf" "$fs"   && echo "ok   distro.conf is checked before anything is created"   || { echo "FAIL conf check ($conf) before create_filesystems ($fs)"; fails=$((fails + 1)); }
order_ok "$pre" "$cp_"   && echo "ok   pre-copy runs before the copy"                        || { echo "FAIL pre-copy ($pre) before copy ($cp_)"; fails=$((fails + 1)); }
order_ok "$post" "$dracut" && echo "ok   post-copy runs before the initramfs is rebuilt"     || { echo "FAIL post-copy ($post) before dracut ($dracut)"; fails=$((fails + 1)); }
order_ok "$rm_" "$fin"   && echo "ok   post-install runs after the installer is removed"    || { echo "FAIL post-install ($fin) after removal ($rm_)"; fails=$((fails + 1)); }
order_ok "$fin" "$umount_" && echo "ok   and before the target is unmounted"                 || { echo "FAIL post-install ($fin) before umount ($umount_)"; fails=$((fails + 1)); }

[ "$fails" -eq 0 ]
