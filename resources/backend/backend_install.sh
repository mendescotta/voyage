#!/bin/bash
# Installation logic extracted from void-installer

# --- 1. ENVIRONMENT SETUP ---

CONF_FILE="/tmp/.void-installer.conf"
TARGETDIR="/mnt/target"
LOG="/tmp/installation.log"
TARGET_FSTAB=$(mktemp -t vinstall-fstab-XXXXXXXX || exit 1)

# fd3 keeps a path to the real stdout so log_ui's ">>>" markers reach the
# Python frontend even after stdout/stderr get redirected to LOG below.
exec 3>&1
exec > >(tee -a "$LOG") 2>&1

log_ui() {
    echo ">>> $1" >&3
}

die() {
    log_ui "ERROR: $1"
    echo "FATAL ERROR: $1" >&2
    umount -R "$TARGETDIR" >/dev/null 2>&1
    exit 1
}

if [ -e /sys/firmware/efi/systab ]; then
    EFI_SYSTEM=1
    EFI_FW_BITS=$(cat /sys/firmware/efi/fw_platform_size)
    if [ $EFI_FW_BITS -eq 32 ]; then
        EFI_TARGET=i386-efi
    else
        EFI_TARGET=x86_64-efi
    fi
fi

# Detect the live environment's init: copy_rootfs copies that same
# environment to disk, so whatever init is detected here is what ends up
# installed.
if [ -x /sbin/dynamod-init ]; then
    INIT_SYSTEM="dynamod"
elif [ -x /sbin/dinit ] || [ -x /usr/bin/dinit ]; then
    INIT_SYSTEM="dinit"
else
    INIT_SYSTEM="runit"
fi

# --- 2. UTILITY FUNCTIONS ---

get_option() {
    grep -E "^${1} .*" "$CONF_FILE" | sed -e "s|^${1} ||"
}

enable_service() {
    case "$INIT_SYSTEM" in
        dinit)
            ln -sf "/etc/dinit.d/$1" "$TARGETDIR/etc/dinit.d/boot.d/$1"
            ;;
        dynamod)
            # dynamod enables services by presence in /etc/dynamod/services/,
            # not by symlink. Services the dynamod package ships are already
            # there; anything else this is asked to enable needs its own
            # .toml written by the caller first -- this can't invent one
            # from a dinit/runit service name alone.
            [ -f "$TARGETDIR/etc/dynamod/services/$1.toml" ] || \
                echo "WARNING: no /etc/dynamod/services/$1.toml found to enable" >&2
            ;;
        *)
            ln -sf "/etc/sv/$1" "$TARGETDIR/etc/runit/runsvdir/default/$1"
            ;;
    esac
}

run_step() {
    local desc="$1"; shift
    echo "-> $desc"
    "$@" || die "$desc failed"
}

# Retries with -t auto (signature probing) if the recorded filesystem
# type no longer matches reality. Not used for swap (swapon takes no -t)
# or btrfs subvolume mounts (a plain fallback there would silently mount
# the wrong subvolume).
mount_with_fallback() {
    local dev="$1" target="$2" fstype="$3"
    echo "-> Mounting $dev as $fstype on $target"
    if mount -t "$fstype" "$dev" "$target" 2>/dev/null; then
        return 0
    fi
    echo "-> $dev did not mount as $fstype, retrying with auto-detection"
    if mount -t auto "$dev" "$target" 2>/dev/null; then
        return 0
    fi
    die "Error mounting $dev on $target (tried $fstype and auto-detection)"
}

# Extra kernel cmdline dynamod needs, appended to whichever bootloader's
# own cmdline construction is in use. rdinit= bypasses the initramfs's own
# /init entirely and execs this path from the initramfs image as PID 1 --
# a stock dracut-generated initramfs (dracut's own /init) doesn't contain
# dynamod-init, so rebuild_initramfs() below replaces the dracut call with
# one that does, for INIT_SYSTEM=dynamod.
dynamod_cmdline_extra() {
    if [ "$INIT_SYSTEM" = "dynamod" ]; then
        printf ' rdinit=/sbin/dynamod-init init=/sbin/dynamod-init'
    fi
}

# --- 3. CORE INSTALLATION FUNCTIONS ---

create_filesystems() {
    local mnts dev mntpt fstype fspassno mkfs size rv uuid

    # Read MOUNTPOINT lines from the config (sorted by mount point)
    mnts=$(grep -E '^MOUNTPOINT .*' "$CONF_FILE" | sort -k 5)

    set -- ${mnts}
    while [ $# -ne 0 ]; do
        dev=$2; fstype=$3; mntpt="$5"; mkfs=$6
        shift 6

        echo "Processing $dev ($fstype) for $mntpt..."

        if [ "$fstype" = "swap" ]; then
            swapoff "$dev" >/dev/null 2>&1
            if [ "$mkfs" -eq 1 ]; then
                echo "Formatting Swap on $dev..."
                mkswap "$dev" || die "Error creating swap on $dev"
            fi
            swapon "$dev" || die "Error activating swap on $dev"
            uuid=$(blkid -o value -s UUID "$dev")
            echo "UUID=$uuid none swap defaults 0 0" >>"$TARGET_FSTAB"
            continue
        fi

        if [ "$mkfs" -eq 1 ]; then
            echo "Formatting $dev as $fstype..."
            case "$fstype" in
                btrfs) MKFS="mkfs.btrfs"; MKFS_FLAGS="-f -q"; modprobe btrfs ;;
                ext2) MKFS="mke2fs"; MKFS_FLAGS="-F -q"; modprobe ext2 ;;
                ext3) MKFS="mke2fs"; MKFS_FLAGS="-F -q -j"; modprobe ext3 ;;
                ext4) MKFS="mke2fs"; MKFS_FLAGS="-F -q -t ext4"; modprobe ext4 ;;
                f2fs) MKFS="mkfs.f2fs"; MKFS_FLAGS="-f -q"; modprobe f2fs ;;
                vfat) MKFS="mkfs.vfat"; MKFS_FLAGS="-F32"; modprobe vfat ;;
                xfs) MKFS="mkfs.xfs"; MKFS_FLAGS="-f -q -i sparse=0"; modprobe xfs ;;
                *) die "File system $fstype not supported" ;;
            esac
            
            $MKFS $MKFS_FLAGS "$dev" || die "Error formatting $dev ($fstype)"
        fi

        if [ "$mntpt" = "/" ]; then
            mkdir -p "$TARGETDIR"

            if [ "$fstype" = "btrfs" ]; then
                local btrfs_flat="$(get_option BTRFS_FLAT)"
                local btrfs_snapshots="$(get_option BTRFS_SNAPSHOTS)"

                if [ "$btrfs_flat" = "1" ]; then
                    echo "Mounting flat BTRFS root..."
                    mount "$dev" "$TARGETDIR" || die "Error mounting flat BTRFS root"
                    uuid=$(blkid -o value -s UUID "$dev")
                    echo "UUID=$uuid / btrfs defaults 0 0" >>"$TARGET_FSTAB"
                    continue
                fi

                echo "Creating BTRFS subvolumes..."

                # Temporary mount without subvol
                mount "$dev" "$TARGETDIR" || die "Error when mounting temporary BTRFS"

                btrfs subvolume create "$TARGETDIR/@" || die "Error creating @"
                btrfs subvolume create "$TARGETDIR/@home" || die "Error creating @home"
                btrfs subvolume create "$TARGETDIR/@log" || die "Error creating @log"
                btrfs subvolume create "$TARGETDIR/@pkg" || die "Error creating @pkg"
                if [ "$btrfs_snapshots" = "1" ]; then
                    btrfs subvolume create "$TARGETDIR/@snapshots" || die "Error creating @snapshots"
                fi

                umount "$TARGETDIR"

                mount -o subvol=@ "$dev" "$TARGETDIR" || die "Error mounting subvol @"

                mkdir -p "$TARGETDIR/home"
                mkdir -p "$TARGETDIR/var/log"
                mkdir -p "$TARGETDIR/var/cache/xbps"

                mount -o subvol=@home "$dev" "$TARGETDIR/home"
                mount -o subvol=@log  "$dev" "$TARGETDIR/var/log"
                mount -o subvol=@pkg  "$dev" "$TARGETDIR/var/cache/xbps"

                uuid=$(blkid -o value -s UUID "$dev")
                echo "UUID=$uuid / btrfs defaults,subvol=@ 0 0" >>"$TARGET_FSTAB"
                echo "UUID=$uuid /home btrfs defaults,subvol=@home 0 0" >>"$TARGET_FSTAB"
                echo "UUID=$uuid /var/log btrfs defaults,subvol=@log 0 0" >>"$TARGET_FSTAB"
                echo "UUID=$uuid /var/cache/xbps btrfs defaults,subvol=@pkg 0 0" >>"$TARGET_FSTAB"

                if [ "$btrfs_snapshots" = "1" ]; then
                    mkdir -p "$TARGETDIR/.snapshots"
                    mount -o subvol=@snapshots "$dev" "$TARGETDIR/.snapshots"
                    echo "UUID=$uuid /.snapshots btrfs defaults,subvol=@snapshots 0 0" >>"$TARGET_FSTAB"
                fi

                continue
            fi

            mount_with_fallback "$dev" "$TARGETDIR" "$fstype"

            uuid=$(blkid -o value -s UUID "$dev")
            if [ "$fstype" = "f2fs" ] || [ "$fstype" = "btrfs" ] || [ "$fstype" = "xfs" ]; then
                fspassno=0
            else
                fspassno=1
            fi
            echo "UUID=$uuid $mntpt $fstype defaults 0 $fspassno" >>"$TARGET_FSTAB"
        fi
    done

    # Mount the remaining partitions (neither root nor swap)
    set -- ${mnts}
    while [ $# -ne 0 ]; do
        dev=$2; fstype=$3; mntpt="$5"
        shift 6
        [ "$mntpt" = "/" ] || [ "$fstype" = "swap" ] && continue
        
        mkdir -p "${TARGETDIR}${mntpt}"
        mount_with_fallback "$dev" "${TARGETDIR}${mntpt}" "$fstype"
        
        uuid=$(blkid -o value -s UUID "$dev")
        if [ "$fstype" = "f2fs" ] || [ "$fstype" = "btrfs" ] || [ "$fstype" = "xfs" ]; then
            fspassno=0
        else
            fspassno=2
        fi
        echo "UUID=$uuid $mntpt $fstype defaults 0 $fspassno" >>"$TARGET_FSTAB"
    done
}

# btrfs needs COW disabled on the file before it has any content, or
# mkswap/swapon fail on it.
setup_swapfile() {
    local root_fs mem_kib mem_mib size_mib
    root_fs="$(findmnt -no FSTYPE "$TARGETDIR")"
    mem_kib="$(awk '/MemTotal/ {print $2}' /proc/meminfo)"
    mem_mib=$(( mem_kib / 1024 ))
    size_mib=$mem_mib
    [ "$size_mib" -gt 8192 ] && size_mib=8192
    [ "$size_mib" -lt 256 ] && size_mib=256

    run_step "Creating ${size_mib}MiB swapfile" touch "$TARGETDIR/swapfile"
    if [ "$root_fs" = "btrfs" ]; then
        chattr +C "$TARGETDIR/swapfile" 2>/dev/null || true
    fi
    run_step "Allocating swapfile" fallocate -l "${size_mib}M" "$TARGETDIR/swapfile"
    chmod 600 "$TARGETDIR/swapfile"
    run_step "Formatting swapfile" mkswap "$TARGETDIR/swapfile"
    echo "/swapfile none swap defaults 0 0" >>"$TARGET_FSTAB"
}

# Copy the base system from the Live ISO (local source)
copy_rootfs() {
    echo "Copying system files..."
    # We use tar as-is from the original to preserve extended attributes
    tar --create --one-file-system --xattrs \
        --checkpoint=2000 --checkpoint-action=echo="Copied %{r}T files..." \
        -f - / | \
        tar --extract --xattrs --xattrs-include='*' --preserve-permissions -f - -C "$TARGETDIR"

    if [ $? -ne 0 ]; then
        die "Error copying rootfs file system"
    fi

    # Post-copy live cleanup
    rm -f "$TARGETDIR/etc/motd" "$TARGETDIR/etc/issue" "$TARGETDIR/usr/sbin/void-installer"
    # Drop the overlay's Install System shortcut from skel before
    # set_useraccount copies it into the real user's home.
    rm -f "$TARGETDIR/etc/skel/Desktop/voyage.desktop"
    # adduser.sh's live-session convenience rule grants the wheel group
    # every polkit action with no authentication at all; strip it (and
    # the matching sudoers drop-in) so the installed system isn't stuck
    # passwordless too.
    rm -f "$TARGETDIR/etc/polkit-1/rules.d/void-live.rules" \
        "$TARGETDIR/etc/sudoers.d/99-void-live"
    # Do not remove sddm.conf, it may hold autologin config we need
    # Remove the live user from the target. noid-mklive's adduser.sh dracut hook
    # writes the actual live username (default "train", overridable with the
    # live.user cmdline arg -- see noid-mklive/dracut/vmklive/adduser.sh) to
    # /etc/default/live.conf, which copy_rootfs just copied onto the target;
    # read it back instead of assuming a fixed name, so the live account
    # (home dir + its hardcoded "voidlinux" password) never survives into
    # the installed system.
    LIVE_USERNAME=""
    if [ -f "$TARGETDIR/etc/default/live.conf" ]; then
        LIVE_USERNAME="$(. "$TARGETDIR/etc/default/live.conf"; echo "$USERNAME")"
    fi
    if [ -n "$LIVE_USERNAME" ]; then
        chroot "$TARGETDIR" userdel -r "$LIVE_USERNAME" >/dev/null 2>&1
        # The live overlay's agetty-tty1 override autologs in as the live
        # user (-a $LIVE_USERNAME); once that account is gone, leaving the
        # reference in place can keep tty1 from spawning a login prompt.
        if [ -f "$TARGETDIR/etc/sv/agetty-tty1/conf" ]; then
            sed -i "s/-a $LIVE_USERNAME //" "$TARGETDIR/etc/sv/agetty-tty1/conf"
        fi
    fi

    # Populate root's home with the same skel dotfiles new users get
    # (.bashrc, .inputrc, .xinitrc, ...); nothing does this otherwise.
    cp "$TARGETDIR"/etc/skel/.[!.]* "$TARGETDIR/root/" 2>/dev/null

    # Make sure the installed system doesn't have 'pam_rootok' enabled
    PAM_FILES="$TARGETDIR/etc/pam.d/su $TARGETDIR/etc/pam.d/login"
    for file in $PAM_FILES; do
        if [ -f "$file" ]; then
            sed -i 's/^auth\s\+sufficient\s\+pam_rootok\.so/#auth sufficient pam_rootok.so/' "$file"
        fi
    done 
}

mount_filesystems() {
    for f in sys proc dev; do
        [ ! -d "$TARGETDIR/$f" ] && mkdir "$TARGETDIR/$f"
        mount --rbind "/$f" "$TARGETDIR/$f"
    done
}

umount_filesystems() {
    local mnts="$(grep -E '^MOUNTPOINT .* swap .*$' "$CONF_FILE" | sort -r -k 5)"
    set -- ${mnts}
    while [ $# -ne 0 ]; do
        local dev=$2; local fstype=$3
        shift 6
        if [ "$fstype" = "swap" ]; then
            swapoff "$dev"
        fi
    done
    umount -R "$TARGETDIR"
}

# --- 4. SYSTEM CONFIGURATION FUNCTIONS ---

set_hostname() {
    local hostname="$(get_option HOSTNAME)"
    echo "${hostname:-void}" > "$TARGETDIR/etc/hostname"
}

set_timezone() {
    local TIMEZONE="$(get_option TIMEZONE)"
    ln -sf "/usr/share/zoneinfo/${TIMEZONE}" "${TARGETDIR}/etc/localtime"
}

set_locale() {
    local LOCALE="$(get_option LOCALE)"
    : "${LOCALE:=C.UTF-8}"
    sed -i -e "s|LANG=.*|LANG=$LOCALE|g" "$TARGETDIR/etc/locale.conf"
    sed -e "/${LOCALE}/s/^\#//" -i "$TARGETDIR/etc/default/libc-locales"
    chroot "$TARGETDIR" xbps-reconfigure -f glibc-locales
}

set_keymap() {
    local KEYMAP="$(get_option KEYMAP)"
    [ -n "$KEYMAP" ] || return 0

    # The identifier comes from a filename detected on the ISO.
    if [[ ! "$KEYMAP" =~ ^[A-Za-z0-9][A-Za-z0-9+._-]*$ ]]; then
        die "Invalid keyboard map: $KEYMAP"
    fi

    # Void Linux applies the console keymap by reading KEYMAP from /etc/rc.conf
    install -d "$TARGETDIR/etc"
    touch "$TARGETDIR/etc/rc.conf"
    if grep -Eq '^[[:space:]]*KEYMAP[[:space:]]*=' "$TARGETDIR/etc/rc.conf"; then
        sed -i -E "s|^[[:space:]]*KEYMAP[[:space:]]*=.*$|KEYMAP=$KEYMAP|" "$TARGETDIR/etc/rc.conf"
    else
        printf '\nKEYMAP=%s\n' "$KEYMAP" >> "$TARGETDIR/etc/rc.conf"
    fi

    # Update vconsole.conf only if the image already uses it; don't create it on Void.
    if [ -f "$TARGETDIR/etc/vconsole.conf" ]; then
        if grep -Eq '^[[:space:]]*KEYMAP[[:space:]]*=' "$TARGETDIR/etc/vconsole.conf"; then
            sed -i -E "s|^[[:space:]]*KEYMAP[[:space:]]*=.*$|KEYMAP=$KEYMAP|" "$TARGETDIR/etc/vconsole.conf"
        else
            printf '\nKEYMAP=%s\n' "$KEYMAP" >> "$TARGETDIR/etc/vconsole.conf"
        fi
    fi

    # Xorg/Wayland don't interpret console keymap names directly.
    local XKB_LAYOUT="$KEYMAP"
    local XKB_VARIANT=""
    case "$KEYMAP" in
        jp106|jp-OADG109A) XKB_LAYOUT="jp" ;;
        kr106|kr) XKB_LAYOUT="kr" ;;
        la-latin1) XKB_LAYOUT="latam" ;;
        es-olpc) XKB_LAYOUT="es"; XKB_VARIANT="olpc" ;;
        es-winkeys) XKB_LAYOUT="es"; XKB_VARIANT="winkeys" ;;
        latam-winkeys) XKB_LAYOUT="latam"; XKB_VARIANT="winkeys" ;;
        fr-afnor) XKB_LAYOUT="fr"; XKB_VARIANT="afnor" ;;
        fr-bepo) XKB_LAYOUT="fr"; XKB_VARIANT="bepo" ;;
        de-mobii) XKB_LAYOUT="de"; XKB_VARIANT="mobii" ;;
        pt-olpc) XKB_LAYOUT="pt"; XKB_VARIANT="olpc" ;;
        jp-winkeys) XKB_LAYOUT="jp"; XKB_VARIANT="winkeys" ;;
        kr-hangul) XKB_LAYOUT="kr"; XKB_VARIANT="hangul" ;;
        kr-winkeys) XKB_LAYOUT="kr"; XKB_VARIANT="winkeys" ;;
        us-winkeys) XKB_LAYOUT="us"; XKB_VARIANT="winkeys" ;;
        uk-winkeys) XKB_LAYOUT="gb"; XKB_VARIANT="winkeys" ;;
        de-winkeys) XKB_LAYOUT="de"; XKB_VARIANT="winkeys" ;;
        fr-winkeys) XKB_LAYOUT="fr"; XKB_VARIANT="winkeys" ;;
        it-winkeys) XKB_LAYOUT="it"; XKB_VARIANT="winkeys" ;;
        pt-winkeys) XKB_LAYOUT="pt"; XKB_VARIANT="winkeys" ;;
        tr-winkeys) XKB_LAYOUT="tr"; XKB_VARIANT="winkeys" ;;
        *-*|*_*) XKB_LAYOUT="${KEYMAP%%[-_]*}" ;;
    esac

    install -d "$TARGETDIR/etc/X11/xorg.conf.d"
    cat > "$TARGETDIR/etc/X11/xorg.conf.d/00-keyboard.conf" <<EOF
Section "InputClass"
    Identifier "system-keyboard"
    MatchIsKeyboard "on"
    Option "XkbLayout" "$XKB_LAYOUT"
    Option "XkbVariant" "$XKB_VARIANT"
EndSection
EOF

    # Wayland - variables for compositors based on libxkbcommon/wlroots
    install -d "$TARGETDIR/etc/profile.d" "$TARGETDIR/etc/environment.d"
    cat > "$TARGETDIR/etc/profile.d/voyage-keyboard.sh" <<EOF
# Keyboard defaults for Wayland compositors using libxkbcommon/wlroots.
export XKB_DEFAULT_MODEL="pc105"
export XKB_DEFAULT_LAYOUT="$XKB_LAYOUT"
export XKB_DEFAULT_VARIANT="$XKB_VARIANT"
export XKB_DEFAULT_OPTIONS=""
EOF
    chmod 0644 "$TARGETDIR/etc/profile.d/voyage-keyboard.sh"

    cat > "$TARGETDIR/etc/environment.d/90-voyage-keyboard.conf" <<EOF
XKB_DEFAULT_MODEL=pc105
XKB_DEFAULT_LAYOUT=$XKB_LAYOUT
XKB_DEFAULT_VARIANT=$XKB_VARIANT
XKB_DEFAULT_OPTIONS=
EOF
    chmod 0644 "$TARGETDIR/etc/environment.d/90-voyage-keyboard.conf"

    # Display managers may start the compositor without going through profile.d
    touch "$TARGETDIR/etc/environment"
    for env_key in XKB_DEFAULT_MODEL XKB_DEFAULT_LAYOUT XKB_DEFAULT_VARIANT XKB_DEFAULT_OPTIONS; do
        case "$env_key" in
            XKB_DEFAULT_MODEL) env_value="pc105" ;;
            XKB_DEFAULT_LAYOUT) env_value="$XKB_LAYOUT" ;;
            XKB_DEFAULT_VARIANT) env_value="$XKB_VARIANT" ;;
            XKB_DEFAULT_OPTIONS) env_value="" ;;
        esac
        if grep -Eq "^[[:space:]]*${env_key}[[:space:]]*=" "$TARGETDIR/etc/environment"; then
            sed -i -E "s|^[[:space:]]*${env_key}[[:space:]]*=.*$|${env_key}=${env_value}|" "$TARGETDIR/etc/environment"
        else
            printf '%s=%s\n' "$env_key" "$env_value" >> "$TARGETDIR/etc/environment"
        fi
    done

    # GNOME/Mutter keeps the keyboard in GSettings and doesn't use Xorg or the variables
    install -d "$TARGETDIR/etc/xdg/autostart" "$TARGETDIR/usr/libexec"
    cat > "$TARGETDIR/usr/libexec/voyage-wayland-keyboard" <<EOF
#!/bin/sh
# Apply the installer-selected keyboard layout in GNOME Wayland sessions.
command -v gsettings >/dev/null 2>&1 || exit 0
[ "\${XDG_CURRENT_DESKTOP:-}" = "GNOME" ] || exit 0
if [ -n "$XKB_VARIANT" ]; then
    gsettings set org.gnome.desktop.input-sources sources "[('xkb', '${XKB_LAYOUT}+${XKB_VARIANT}')]" || exit 0
else
    gsettings set org.gnome.desktop.input-sources sources "[('xkb', '${XKB_LAYOUT}')]" || exit 0
fi
EOF
    chmod 0755 "$TARGETDIR/usr/libexec/voyage-wayland-keyboard"

    cat > "$TARGETDIR/etc/xdg/autostart/voyage-wayland-keyboard.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Voyage keyboard layout
Comment=Apply the selected keyboard layout in GNOME Wayland
Exec=/usr/libexec/voyage-wayland-keyboard
OnlyShowIn=GNOME;
NoDisplay=true
X-GNOME-Autostart-enabled=true
EOF
    chmod 0644 "$TARGETDIR/etc/xdg/autostart/voyage-wayland-keyboard.desktop"

    # KDE Plasma - kxkbrc in skel for new users
    install -d "$TARGETDIR/etc/skel/.config"
    cat > "$TARGETDIR/etc/skel/.config/kxkbrc" <<EOF
[Layout]
DisplayNames=
LayoutList=$XKB_LAYOUT
Options=
ResetOldOptions=true
SwitchMode=Global
Use=true
VariantList=$XKB_VARIANT
EOF
    chmod 0644 "$TARGETDIR/etc/skel/.config/kxkbrc"
}

set_rootpassword() {
    echo "root:$(get_option ROOTPASSWORD)" | chroot "$TARGETDIR" chpasswd -c SHA512
}

set_default_shell() {
    if ! chroot "$TARGETDIR" xbps-query fish-shell >/dev/null 2>&1; then
        chroot "$TARGETDIR" xbps-install -Sy fish-shell >/dev/null 2>&1 || {
            log_ui "Fish is not available (offline install); keeping the default shell."
            return 0
        }
    fi
    grep -q '^/usr/bin/fish$' "$TARGETDIR/etc/shells" 2>/dev/null || \
        echo /usr/bin/fish >> "$TARGETDIR/etc/shells"
    chroot "$TARGETDIR" usermod -s /usr/bin/fish root
    local USERLOGIN="$(get_option USERLOGIN)"
    [ -n "$USERLOGIN" ] && chroot "$TARGETDIR" usermod -s /usr/bin/fish "$USERLOGIN"
}

set_useraccount() {
    local USERLOGIN="$(get_option USERLOGIN)"
    if [ -n "$USERLOGIN" ]; then
        chroot "$TARGETDIR" useradd -m -G "$(get_option USERGROUPS)" \
            -c "$(get_option USERNAME)" "$USERLOGIN"
        echo "$USERLOGIN:$(get_option USERPASSWORD)" | \
            chroot "$TARGETDIR" chpasswd -c SHA512

        if [ -d "$TARGETDIR/etc/sudoers.d" ]; then
            if [[ "$(get_option USERGROUPS)" != *"wheel"* ]]; then
                echo "$USERLOGIN ALL=(ALL:ALL) ALL" > "$TARGETDIR/etc/sudoers.d/$USERLOGIN"
            else
                echo "%wheel ALL=(ALL:ALL) ALL" > "$TARGETDIR/etc/sudoers.d/wheel"
            fi
        fi
    fi
}

# Configures automatic login on the supported display manager.
set_autologin() {
    local enabled="$(get_option AUTOLOGIN)"
    local manager="$(get_option DISPLAYMANAGER)"
    local userlogin="$(get_option USERLOGIN)"

    [ "$enabled" = "1" ] || return 0

    # Autologin is useless if the display manager itself never starts at
    # boot -- the installer never enables it anywhere else, so do it here.
    case "$manager" in
        sddm|lightdm|gdm)
            enable_service "$manager"
            ;;
    esac

    case "$manager" in
        sddm)
            install -d "$TARGETDIR/etc/sddm.conf.d"
            cat > "$TARGETDIR/etc/sddm.conf.d/10-voyage-autologin.conf" <<EOF
[Autologin]
User=$userlogin
Session=default.desktop
Relogin=false
EOF
            ;;
        lightdm)
            install -d "$TARGETDIR/etc/lightdm/lightdm.conf.d"
            cat > "$TARGETDIR/etc/lightdm/lightdm.conf.d/50-voyage-autologin.conf" <<EOF
[Seat:*]
autologin-user=$userlogin
autologin-user-timeout=0
EOF
            ;;
        gdm)
            if [ -d "$TARGETDIR/etc/gdm" ]; then
                install -d "$TARGETDIR/etc/gdm"
                target_gdm_config="$TARGETDIR/etc/gdm/custom.conf"
            else
                install -d "$TARGETDIR/etc/gdm3"
                target_gdm_config="$TARGETDIR/etc/gdm3/custom.conf"
            fi
            if [ ! -f "$target_gdm_config" ]; then
                printf '%s\n' "[daemon]" > "$target_gdm_config"
            elif ! grep -q '^\[daemon\]' "$target_gdm_config"; then
                printf '\n%s\n' "[daemon]" >> "$target_gdm_config"
            fi
            if grep -q '^AutomaticLoginEnable=' "$target_gdm_config"; then
                sed -i "s/^AutomaticLoginEnable=.*/AutomaticLoginEnable=true/" "$target_gdm_config"
            else
                sed -i '/^\[daemon\]/a AutomaticLoginEnable=true' "$target_gdm_config"
            fi
            if grep -q '^AutomaticLogin=' "$target_gdm_config"; then
                sed -i "s/^AutomaticLogin=.*/AutomaticLogin=$userlogin/" "$target_gdm_config"
            else
                sed -i "/^AutomaticLoginEnable=/a AutomaticLogin=$userlogin" "$target_gdm_config"
            fi
            ;;
        greetd)
            echo "Autologin not configured: greetd is not supported by this installer."
            ;;
        *)
            echo "Autologin not configured: no supported display manager was detected."
            ;;
    esac
}

declare -A MIRRORS

# Format: ["logical-name"]="URL"
MIRRORS["Default"]="https://repo-default.voidlinux.org/"
MIRRORS["Finland"]="https://repo-fi.voidlinux.org/"
MIRRORS["Germany"]="https://repo-de.voidlinux.org/"
MIRRORS["Global"]="https://repo-fastly.voidlinux.org/"
MIRRORS["USA"]="https://mirrors.summithq.com/voidlinux/"

set_mirror() {
    local MIRROR_KEY="$(get_option MIRROR)"
    local MIRROR_URL=${MIRRORS[$MIRROR_KEY]}

    # Only configure a mirror if it's not the local ISO
    if [[ "$MIRROR_KEY" != "Local" ]] && [[ -n "$MIRROR_URL" ]]; then
        echo "Configuring mirror..."
        log_ui "MIRROR"        
        
        if ! chroot "$TARGETDIR" xmirror -s "$MIRROR_URL"; then
            die "Error configuring mirror $MIRROR_KEY ($MIRROR_URL)"
        fi
        echo "Mirror configured: $MIRROR_KEY ($MIRROR_URL)"
    fi
}

update_system() {
    echo "Downloading system updates..."
    log_ui "UPDATE_DOWNLOAD"

    # First phase: download to cache, without installing yet.
    if ! chroot "$TARGETDIR" xbps-install -Suy -d; then
        die "Error downloading system updates"
    fi

    echo "Installing downloaded updates..."
    log_ui "UPDATE_INSTALL"
    if ! chroot "$TARGETDIR" xbps-install -uy; then
        die "Error installing system updates"
    fi
    echo "System updated"
}

enable_nonfree_repos() {
    echo "Enabling non-free repositories..."
    log_ui "NON-FREE"

    chroot "$TARGETDIR" xbps-install -Sy void-repo-nonfree || die "Error installing void-repo-nonfree"
    echo "Non-free repositories enabled"
}

get_nvidia_driver() {
    local info
    info=$(lspci | grep -i nvidia)

    local series
    series=$(echo "$info" | grep -oP '\b[0-9]{3,}\b' | head -n1)

    local driver=""

    if [ -n "$series" ]; then
        if [ "$series" -ge 800 ]; then
            driver="nvidia"
        elif [ "$series" -ge 600 ]; then
            driver="nvidia470"
        elif [ "$series" -ge 400 ]; then
            driver="nvidia390"
        fi
    fi
    echo "$driver"
}

install_nvidia_driver() {
    local driver
    driver=$(get_nvidia_driver)

    if [ -n "$driver" ]; then
        echo "Installing NVIDIA driver: $driver..."
        log_ui "NVIDIA"
        chroot "$TARGETDIR" xbps-install -Sy "$driver" || die "Error installing driver $driver"
        echo "NVIDIA driver successfully installed"
    else
        echo "No compatible NVIDIA driver detected, nouveau/nvk will be used"
    fi
}

install_intel_microcodes() {
    echo "Installing Intel microcode..."
    log_ui "INTEL"
    chroot "$TARGETDIR" xbps-install -Sy intel-ucode || die "Failure when installing Intel microcode"
    echo "Intel microcodes installed correctly"
}

install_extra_software() {
    local update=$(get_option UPDATE)   # GUI checkbox
    local nonfree=$(get_option NONFREE)
    local nvidia=$(get_option NVIDIA)
    local intel=$(get_option INTEL)

    if [ "$update" = "1" ]; then
        update_system
        if [ "$nonfree" = "1" ]; then
            enable_nonfree_repos
            if [ "$nvidia" = "1" ]; then
                install_nvidia_driver
            fi

            if [ "$intel" = "1" ]; then
                install_intel_microcodes
            fi
        else
            echo "Non-free repositories and proprietary drivers were not activated"
        fi
    else
        echo "Offline installer: the system will not be updated"
    fi
}

# Root device/UUID and detected kernel version, shared by the limine and
# refind installers (grub-mkconfig works this out on its own via
# /etc/grub.d/10_linux, so install_grub doesn't need these).
root_partition_dev() {
    grep -E '^MOUNTPOINT .*' "$CONF_FILE" | awk '$5 == "/" {print $2}'
}

esp_partition_dev() {
    grep -E '^MOUNTPOINT .*' "$CONF_FILE" | awk '$5 == "/boot/efi" {print $2}'
}

kernel_version() {
    basename "$(ls "$TARGETDIR"/boot/vmlinuz-* 2>/dev/null | head -1)" | sed 's/^vmlinuz-//'
}

ensure_package() {
    chroot "$TARGETDIR" xbps-query "$1" >/dev/null 2>&1 || \
        chroot "$TARGETDIR" xbps-install -Sy "$1" || die "Error installing $1"
}

install_grub() {
    local dev="$1" grub_args=""

    # GRUB branding is configured exclusively in /etc/default/grub.
    install -d "$TARGETDIR/etc/default"
    if [ -f "$TARGETDIR/etc/default/grub" ]; then
        if grep -q '^GRUB_DISTRIBUTOR=' "$TARGETDIR/etc/default/grub"; then
            sed -i 's|^GRUB_DISTRIBUTOR=.*|GRUB_DISTRIBUTOR="Void"|' "$TARGETDIR/etc/default/grub"
        else
            printf '\nGRUB_DISTRIBUTOR="Void"\n' >> "$TARGETDIR/etc/default/grub"
        fi
    else
        printf 'GRUB_DISTRIBUTOR="Void"\n' > "$TARGETDIR/etc/default/grub"
    fi

    if [ -n "$EFI_SYSTEM" ]; then
        grub_args="--target=$EFI_TARGET --efi-directory=/boot/efi --bootloader-id=Void --recheck"
    fi

    # grub-mkconfig's own /etc/grub.d/10_linux builds the cmdline from
    # GRUB_CMDLINE_LINUX_DEFAULT -- set it before generating grub.cfg
    # rather than patching the generated file after the fact.
    local extra_cmdline
    extra_cmdline="$(dynamod_cmdline_extra)"
    if [ -n "$extra_cmdline" ]; then
        if grep -q '^GRUB_CMDLINE_LINUX_DEFAULT=' "$TARGETDIR/etc/default/grub"; then
            sed -i "s|^GRUB_CMDLINE_LINUX_DEFAULT=\"\\(.*\\)\"|GRUB_CMDLINE_LINUX_DEFAULT=\"\\1${extra_cmdline}\"|" \
                "$TARGETDIR/etc/default/grub"
        else
            printf 'GRUB_CMDLINE_LINUX_DEFAULT="%s"\n' "${extra_cmdline# }" >> "$TARGETDIR/etc/default/grub"
        fi
    fi

    chroot "$TARGETDIR" grub-install $grub_args "$dev" || die "Error installing GRUB on $dev"
    chroot "$TARGETDIR" grub-mkconfig -o /boot/grub/grub.cfg || die "Error generating grub.cfg"
}

install_limine() {
    local kver root_uuid esp_dir

    # Recent Limine dropped ext2/3/4 (and never supported btrfs) for reading
    # its own config/kernel/initramfs, so those must live on a FAT32
    # partition. Voyage only has one to offer: the EFI System Partition -- so
    # Limine is EFI-only here, with everything placed at the ESP's root.
    [ -n "$EFI_SYSTEM" ] || die "Limine requires an EFI system partition"

    ensure_package limine
    kver="$(kernel_version)"
    [ -n "$kver" ] || die "No kernel image found in $TARGETDIR/boot for Limine"
    root_uuid="$(blkid -o value -s UUID "$(root_partition_dev)")"

    esp_dir="$TARGETDIR/boot/efi"
    install -d "$esp_dir/EFI/BOOT"
    cp "$TARGETDIR/boot/vmlinuz-$kver" "$esp_dir/vmlinuz-$kver" || die "Error copying kernel for Limine"
    cp "$TARGETDIR/boot/initramfs-$kver.img" "$esp_dir/initramfs-$kver.img" || die "Error copying initramfs for Limine"
    cp "$TARGETDIR/usr/share/limine/BOOTX64.EFI" "$esp_dir/EFI/BOOT/BOOTX64.EFI" \
        || die "Error installing the Limine EFI binary"

    cat > "$esp_dir/limine.cfg" <<EOF
INTERFACE_BRANDING=Void Linux
TIMEOUT=5

:Boot with standard options
    PROTOCOL=linux
    KERNEL_PATH=boot:///vmlinuz-$kver
    MODULE_PATH=boot:///initramfs-$kver.img
    CMDLINE=root=UUID=$root_uuid rw$(dynamod_cmdline_extra)
EOF
}

install_refind() {
    local root_uuid esp_dev

    [ -n "$EFI_SYSTEM" ] || die "rEFInd requires an EFI system partition"

    ensure_package refind
    root_uuid="$(blkid -o value -s UUID "$(root_partition_dev)")"
    esp_dev="$(esp_partition_dev)"
    [ -n "$esp_dev" ] || die "No EFI system partition found for rEFInd"

    local refind_dir="$TARGETDIR/boot/efi/EFI/refind"

    # refind-install only writes its default refind.conf when one isn't
    # already there; on a disk/ESP reused across install attempts, a
    # stale refind.conf (in our case, a previous run's own one-line
    # include-only file) makes it skip that step entirely. Remove it
    # first so every install run gets a real default config.
    rm -f "$refind_dir/refind.conf"

    chroot "$TARGETDIR" refind-install --usedefault "$esp_dev" || die "Error installing rEFInd"

    local theme_src="$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/refind-theme/RONBM"
    if [ -d "$theme_src" ]; then
        install -d "$refind_dir/themes/RONBM"
        cp -a "$theme_src/." "$refind_dir/themes/RONBM/"
        echo "include themes/RONBM/theme.conf" >> "$refind_dir/refind.conf"
    else
        echo "WARNING: RONBM theme not found at $theme_src, rEFInd will use its default theme"
    fi

    # initrd= path is relative to the filesystem root, not /boot, since
    # /boot is never its own mountpoint here -- needs the /boot/ prefix.
    local extra_cmdline
    extra_cmdline="$(dynamod_cmdline_extra)"
    cat > "$TARGETDIR/boot/refind_linux.conf" <<EOF
"Boot with standard options"  "root=UUID=$root_uuid rw${extra_cmdline} initrd=/boot/initramfs-%v.img"
"Boot to single-user mode"  "root=UUID=$root_uuid rw single${extra_cmdline} initrd=/boot/initramfs-%v.img"
EOF
}

set_bootloader() {
    local dev="$(get_option BOOTLOADER)" bl_type="$(get_option BOOTLOADER_TYPE)"

    if [ "$dev" = "none" ] || [ -z "$dev" ]; then return; fi

    # grub is a hard `depends` of the voyage package itself
    # (needed regardless of which bootloader the user ends up picking),
    # so it's always present on the target here. Remove whichever
    # bootloader packages weren't chosen so their kernel hooks (grub.cfg
    # regeneration, etc.) don't keep firing for the rest of the install.
    case "${bl_type:-grub}" in
        limine) install_limine; chroot "$TARGETDIR" xbps-pkgdb -m manual limine
                chroot "$TARGETDIR" xbps-remove -y grub refind 2>/dev/null || true ;;
        refind) install_refind; chroot "$TARGETDIR" xbps-pkgdb -m manual refind
                chroot "$TARGETDIR" xbps-remove -y grub limine 2>/dev/null || true ;;
        *)      install_grub "$dev"; chroot "$TARGETDIR" xbps-pkgdb -m manual grub
                chroot "$TARGETDIR" xbps-remove -y limine refind 2>/dev/null || true ;;
    esac
}

# --- 5. MAIN ORCHESTRATION ---

# Preliminary validations
if [ "$(id -u)" != "0" ]; then
    echo "This script must be run as root." >&2
    exit 1
fi

if [ ! -f "$CONF_FILE" ]; then
    die "$CONF_FILE was not found. The Python frontend must generate it first."
fi

log_ui "INIT"
echo "Log started at $LOG"

# Step 1: Disks
log_ui "CREATE_FS"
create_filesystems
if [ "$(get_option SWAPTYPE)" = "swapfile" ]; then
    setup_swapfile
fi

# Step 2: Base install
log_ui "COPY"
copy_rootfs

# Step 3: Configuration
log_ui "REGIONAL_CONFIG"
mount_filesystems
install -Dm644 "$TARGET_FSTAB" "$TARGETDIR/etc/fstab"
echo "tmpfs /tmp tmpfs defaults,nosuid,nodev 0 0" >> "$TARGETDIR/etc/fstab"
touch "$TARGETDIR/etc/voyage-installer-release"

set_keymap
set_locale
set_timezone
set_hostname

# Mirrors and proprietary drivers
set_mirror
install_extra_software

# Rebuild the initramfs for the target now that any extra drivers
# (nvidia/intel) are in place -- the live ISO's initramfs was built for
# the live environment's hardware, not necessarily the install target's.
echo "Rebuilding initramfs for the target system..."
if [ "$INIT_SYSTEM" = "dynamod" ]; then
    # rdinit= bypasses dracut's own /init entirely -- a stock
    # dracut-generated initramfs never contains dynamod-init, so build a
    # cpio image with it directly instead, at the same
    # initramfs-<kver>.img path dracut would have used (every bootloader
    # function above already looks for that exact filename, so nothing
    # else needs to change to find it).
    kver="$(kernel_version)"
    [ -n "$kver" ] || die "No kernel image found in $TARGETDIR/boot to rebuild the initramfs for"
    initramfs_dir="$(mktemp -d)"
    mkdir -p "$initramfs_dir"/{sbin,bin,dev,proc,sys,newroot}
    cp "$TARGETDIR/sbin/dynamod-init" "$initramfs_dir/sbin/dynamod-init"
    if [ -f "$TARGETDIR/usr/bin/busybox" ]; then
        cp "$TARGETDIR/usr/bin/busybox" "$initramfs_dir/bin/busybox"
        for cmd in sh mdev mount umount; do
            ln -sf busybox "$initramfs_dir/bin/$cmd"
        done
        ln -sf ../bin/mdev "$initramfs_dir/sbin/mdev"
    fi
    ( cd "$initramfs_dir" && find . -print0 | cpio --null -o --format=newc 2>/dev/null | gzip -9 ) \
        > "$TARGETDIR/boot/initramfs-$kver.img"
    rm -rf "$initramfs_dir"
else
    chroot "$TARGETDIR" dracut --no-hostonly --add-drivers "ahci" --force || die "Error rebuilding initramfs"
fi

log_ui "USER_CONFIG"
set_rootpassword
set_useraccount
set_autologin
set_default_shell

# Step 4: Bootloader
log_ui "GRUB_INSTALL"
set_bootloader

# Step 5: Finish
echo "Removing the installer and orphaned packages/cache..."
chroot "$TARGETDIR" xbps-remove -ROoy voyage xmirror dialog xtools-minimal

log_ui "FINISH"
sync
install -d "$TARGETDIR/var/log"
cp "$LOG" "$TARGETDIR/var/log/voyage-install.log" 2>/dev/null || \
    echo "WARNING: could not copy install log into the target system" >&2
umount_filesystems
rm -f "$TARGET_FSTAB"


log_ui "DONE"

exit 0
