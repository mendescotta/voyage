# Sourced by backend_install.sh. Distro policy lives in data, not in the installer:
#   - distro.conf  (shipped next to the scripts; /etc/voyage/distro.conf in the live image replaces it)
#   - hooks        (executables in <backend>/hooks.d/<stage>/ and /etc/voyage/hooks.d/<stage>/)
# Hooks run as root, so they are held to strict rules (run_hooks).
#
# distro.conf: one `key value...` per line, `#` starts a comment. Unknown keys are an error.
#   shell <name> <absolute path> [package]         a login shell the installer may offer (package installed if missing)
#   default-shell <name>                           the shell both accounts get unless the user picks another
#   vbox-guest-install <package>...                installed when hardware setup detects a VirtualBox guest
#   vbox-guest-enable <service>...                 services switched on with them
#   install <package>...                           extra packages for the new system (online installs only)
#   remove <package>...                            packages removed from the new system (the installer itself)
#   enable <service>...  /  disable <service>...   services of the new system, by the init's own names

DISTRO_CONF_OVERRIDE="${DISTRO_CONF_OVERRIDE:-/etc/voyage/distro.conf}"
HOOKS_TRUSTED_UID="${HOOKS_TRUSTED_UID:-0}"

# the file in effect: the override replaces the shipped one entirely (no merging, so nothing surprises)
distro_conf_file() {
    if [ -f "$DISTRO_CONF_OVERRIDE" ]; then
        echo "$DISTRO_CONF_OVERRIDE"
    else
        echo "${DISTRO_CONF_DEFAULT:-}"
    fi
}

# every word after <key>, one per line, across all lines of that key
distro_values() {
    local f
    f="$(distro_conf_file)"
    [ -f "$f" ] || return 0
    awk -v key="$1" '{ sub(/#.*/, "") } $1 == key { for (i = 2; i <= NF; i++) print $i }' "$f"
}

distro_value() {
    distro_values "$1" | head -n 1
}

# the names of the shells the config offers, one per line, in file order
distro_shell_names() {
    local f
    f="$(distro_conf_file)"
    [ -f "$f" ] || return 0
    awk '{ sub(/#.*/, "") } $1 == "shell" { print $2 }' "$f"
}

# "path [package]" of a shell by name; fails when the config does not define it
distro_shell_info() {
    local f
    f="$(distro_conf_file)"
    [ -f "$f" ] || return 1
    awk -v name="$1" '
        { sub(/#.*/, "") }
        $1 == "shell" && $2 == name { print $3 (NF >= 4 ? " " $4 : ""); found = 1; exit }
        END { exit (found ? 0 : 1) }' "$f"
}

# Fails (with file:line: reason on stderr) on an unknown key or an unsafe name. Run before anything is touched.
distro_conf_check() {
    local f
    f="$(distro_conf_file)"
    [ -f "$f" ] || return 0
    if [ "$f" = "$DISTRO_CONF_OVERRIDE" ] && ! _hook_trusted "$f"; then
        echo "$f: the override must be owned by uid $HOOKS_TRUSTED_UID and not writable by others" >&2
        return 1
    fi
    if [ "$f" = "$DISTRO_CONF_OVERRIDE" ] && ! grep -qE '^[[:space:]]*remove[[:space:]]' "$f"; then
        echo "note: $f has no 'remove' line, so the installer packages stay on the new system" >&2
    fi
    awk -v file="$f" '
        function bad(msg) { printf "%s:%d: %s\n", file, NR, msg > "/dev/stderr"; rc = 1 }
        { sub(/#.*/, "") }
        NF == 0 { next }
        {
            key = $1
            if (key == "shell") {
                if (NF < 3 || NF > 4) bad("shell takes a name, an absolute path and an optional package")
                else if ($2 !~ /^[a-z][a-z0-9-]*$/) bad("invalid shell name (lowercase letters, digits, dashes): " $2)
                else if ($3 !~ /^\/[A-Za-z0-9._+\/-]+$/ || $3 ~ /\.\.|\/\//) bad("a shell needs a plain absolute path (no .. or //): " $3)
                else if (NF == 4 && $4 !~ /^[A-Za-z0-9][A-Za-z0-9._+-]*$/) bad("invalid package name: " $4)
                else shells[$2] = 1
            } else if (key == "default-shell") {
                if (NF != 2) bad("default-shell takes exactly one shell name")
                else { dflt = $2; dflt_line = NR }
            } else if (key == "install" || key == "remove" || key == "enable" || key == "disable" || key == "vbox-guest-install" || key == "vbox-guest-enable") {
                if (NF < 2) bad(key " needs at least one name")
                for (i = 2; i <= NF; i++)
                    if ($i !~ /^[A-Za-z0-9][A-Za-z0-9._+-]*$/) bad("invalid name for " key ": " $i)
            } else {
                bad("unknown key: " key)
            }
        }
        END {
            if (dflt != "" && !(dflt in shells)) {
                printf "%s:%d: default-shell names a shell that no shell line defines: %s\n", file, dflt_line, dflt > "/dev/stderr"
                rc = 1
            }
            exit rc
        }' "$f"
}

# A path may feed root only if the trusted user owns it and nobody else can write to it...
_hook_self_trusted() {
    local path="$1" owner mode
    read -r owner mode < <(stat -c '%u %a' "$path" 2>/dev/null) || return 1
    [ "$owner" = "$HOOKS_TRUSTED_UID" ] || return 1
    [ $(( 8#$mode & 8#022 )) -eq 0 ]
}

# ...and so must every directory above it (symlinks are resolved first). Someone who can write to a parent
# can swap the whole tree between the check and the execution. A sticky shared directory such as /tmp is
# accepted when the entry below it is ours: nobody else can rename or remove that.
_hook_chain_trusted() {
    local path child owner mode child_owner
    path="$(readlink -f -- "$1" 2>/dev/null)" || return 1
    [ -n "$path" ] || return 1
    child="$path"
    while [ "$path" != "/" ]; do
        path="$(dirname "$path")"
        read -r owner mode < <(stat -c '%u %a' "$path" 2>/dev/null) || return 1
        [ "$owner" = "0" ] || [ "$owner" = "$HOOKS_TRUSTED_UID" ] || return 1
        if [ $(( 8#$mode & 8#022 )) -ne 0 ]; then
            [ $(( 8#$mode & 8#1000 )) -ne 0 ] || return 1
            child_owner="$(stat -c '%u' "$child" 2>/dev/null)" || return 1
            [ "$child_owner" = "$HOOKS_TRUSTED_UID" ] || return 1
        fi
        child="$path"
    done
    return 0
}

_hook_trusted() {
    _hook_self_trusted "$1" && _hook_chain_trusted "$1"
}

# run_hooks <stage>: runs the executables of <dir>/<stage>/ for every hooks directory, in name order.
# A hook in a later directory (/etc/voyage/hooks.d) replaces a same-named one in an earlier (shipped) one.
# Everything is checked before the first hook runs. A failing hook fails the stage unless its name ends
# in .optional; *.disabled is skipped. Hooks get VOYAGE_STAGE, TARGETDIR, INIT_SYSTEM and CONF_FILE.
run_hooks() {
    local stage="$1" dir path name
    local -a dirs=() names=()
    local -A chosen=()
    case "$stage" in
        "" | *[!A-Za-z0-9_-]*) echo "hooks: invalid stage name: $stage" >&2; return 1 ;;
    esac
    # HOOKS_DIRS is a colon-separated list; the default is the shipped directory, then /etc
    IFS=: read -r -a dirs <<<"${HOOKS_DIRS:-$(dirname "${BASH_SOURCE[0]}")/hooks.d:/etc/voyage/hooks.d}"
    for dir in "${dirs[@]}"; do
        [ -d "$dir/$stage" ] || continue
        if ! _hook_trusted "$dir/$stage"; then
            echo "hooks: refusing $dir/$stage: not owned by uid $HOOKS_TRUSTED_UID or writable by others" >&2
            return 1
        fi
        for path in "$dir/$stage"/*; do
            [ -e "$path" ] || continue
            name="${path##*/}"
            [[ $name =~ ^[0-9A-Za-z][0-9A-Za-z._-]*$ ]] || { echo "hooks: skipping oddly named $path" >&2; continue; }
            case "$name" in
                # NAME.disabled switches NAME off, also when NAME comes from an earlier (shipped) directory
                *.disabled) unset "chosen[${name%.disabled}]"; continue ;;
            esac
            # a same-named file that is not executable shadows (switches off) the earlier one
            if [ ! -x "$path" ]; then unset "chosen[$name]"; continue; fi
            chosen[$name]="$path"
        done
    done
    [ ${#chosen[@]} -gt 0 ] || return 0
    mapfile -t names < <(printf '%s\n' "${!chosen[@]}" | sort)
    for name in "${names[@]}"; do
        path="${chosen[$name]}"
        if [ -L "$path" ] || [ ! -f "$path" ] || ! _hook_trusted "$path"; then
            echo "hooks: refusing $path: must be a regular file owned by uid $HOOKS_TRUSTED_UID that others cannot write" >&2
            return 1
        fi
    done
    for name in "${names[@]}"; do
        path="${chosen[$name]}"
        echo "-> hook $stage/$name"
        if ! env VOYAGE_STAGE="$stage" TARGETDIR="${TARGETDIR:-}" INIT_SYSTEM="${INIT_SYSTEM:-}" \
                CONF_FILE="${CONF_FILE:-}" "$path"; then
            case "$name" in
                *.optional) echo "hook $stage/$name failed (optional): continuing" >&2 ;;
                *) echo "hook $stage/$name failed" >&2; return 1 ;;
            esac
        fi
    done
    return 0
}
