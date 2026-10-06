# voyage

A GTK4 installer for Void Linux. It drives the `void-installer` shell backend (partitioning,
rootfs copy, bootloader, users) from a tabbed UI: Welcome, Mirror and Software, Users, Disks, Review.

- Bootloaders: GRUB, Limine, rEFInd (with the RONBM theme).
- Drivers: [voidhw](https://github.com/mendescotta/voidhw) detects the hardware and installs the right GPU driver, firmware, microcode and VM guest tools. The initramfs can be generic (any hardware) or targeted (this machine only).
- Plain GTK4 by default; `--features adwaita` builds with libadwaita.
- `voyage --demo` walks the UI without touching disks.

```
cargo build --release && cargo test
```

The backend is `resources/backend/` (found next to the binary under `share/voyage/backend`, or via
`VOYAGE_BACKEND_DIR`). Tests: `cargo test` and `tests/*.sh`.

## References

- [void-mklive](https://github.com/void-linux/void-mklive): `installer.sh`, the backend this installer wraps and adapts.
- [Kasha Installer](https://codeberg.org/Crow_rei/Kasha-Installer): the top tab bar layout.
- [rEFInd](https://www.rodsbooks.com/refind/) and the [RONBM theme](https://github.com/gutlessCGH) (MIT, bundled in `resources/backend/refind-theme/`).
- [gtk4-rs](https://gtk-rs.org/) and [libadwaita-rs](https://world.pages.gitlab.gnome.org/Rust/libadwaita-rs/).

Licensed under GPL-3.0-or-later.
