# Sourced by backend_install.sh. Distro policy lives in data, not in the installer:
#   - distro.conf  (shipped next to the scripts; /etc/voyage/distro.conf in the live image replaces it)
#   - hooks        (executables in <backend>/hooks.d/<stage>/ and /etc/voyage/hooks.d/<stage>/)
# Hooks run as root, so they are held to strict rules (run_hooks).
#
# distro.conf: one `key value...` per line, `#` starts a comment. Unknown keys are an error.
#   default-shell <absolute path|none> [package]   login shell of root and the first user
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

# Fails (with file:line: reason on stderr) on an unknown key or an unsafe name. Run before anything is touched.
distro_conf_check() {
    local f
    f="$(distro_conf_file)"
    [ -f "$f" ] || return 0
    if [ "$f" = "$DISTRO_CONF_OVERRIDE" ] && ! _hook_trusted "$f"; then
        echo "$f: the override must be owned by uid $HOOKS_TRUSTED_UID and not writable by others" >&2
        return 1
    fi
    awk -v file="$f" '
        function bad(msg) { printf "%s:%d: %s\n", file, NR, msg > "/dev/stderr"; rc = 1 }
        { sub(/#.*/, "") }
        NF == 0 { next }
        {
            key = $1
            if (key == "default-shell") {
                if (NF < 2 || NF > 3) bad("default-shell takes a path (or none) and an optional package")
                else if ($2 != "none" && ($2 !~ /^\/[A-Za-z0-9._+\/-]+$/ || $2 ~ /\.\.|\/\//)) bad("default-shell needs a plain absolute path (no .. or //) or none: " $2)
                else if (NF == 3 && $3 !~ /^[A-Za-z0-9][A-Za-z0-9._+-]*$/) bad("invalid package name: " $3)
            } else if (key == "install" || key == "remove" || key == "enable" || key == "disable") {
                if (NF < 2) bad(key " needs at least one name")
                for (i = 2; i <= NF; i++)
                    if ($i !~ /^[A-Za-z0-9][A-Za-z0-9._+-]*$/) bad("invalid name for " key ": " $i)
            } else {
                bad("unknown key: " key)
            }
        }
        END { exit rc }' "$f"
}

# A path may feed root only if the trusted user owns it and nobody else can write to it.
_hook_trusted() {
    local path="$1" owner mode
    read -r owner mode < <(stat -c '%u %a' "$path" 2>/dev/null) || return 1
    [ "$owner" = "$HOOKS_TRUSTED_UID" ] || return 1
    [ $(( 8#$mode & 8#022 )) -eq 0 ]
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
