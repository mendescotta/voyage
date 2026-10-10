#!/bin/bash
# Use the action explicitly: dinit-shutdown called through a generic reboot
# symlink can otherwise select poweroff. pkexec does not preserve the GUI PATH.
set -eu
if [ "$(cat /proc/1/comm)" = "dinit" ]; then
    exec /usr/bin/dinit-shutdown -r
fi
for reboot in /usr/bin/reboot /usr/sbin/reboot /sbin/reboot /bin/reboot; do
    if [ -x "$reboot" ]; then
        exec "$reboot"
    fi
done
echo "No reboot command is installed." >&2
exit 1
