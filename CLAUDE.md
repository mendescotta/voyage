# voyage

GTK4/Rust installer for Void, built on the official `void-installer` shell
backend. Flagship project. Public repo. Design notes: @DEVNOTES.md (local, gitignored).

## Check before claiming done
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- UI without touching disks: `cargo run -- --demo`.

## Rules
- Backend is the shell scripts in `resources/backend` (`backend_install.sh`,
  `auto_partition.sh`); the GUI is unprivileged and spawns `pkexec`.
  `backend::config_schema` and `install_runner` stay GTK-free and tested.
- Dev env vars: `VOYAGE_BACKEND_DIR` (backend scripts), `VOYAGE_SCREENSHOTS` (screenshot mode in
  `ui/window.rs`), `--demo` (no disk access).
- Destructive steps need two confirmations (auto-partition warning, review page).
- Any change to partitioning, bootloader or install scripts is only
  "done" after a VM install test; say so if it was not run.
- Branches `backup/pre-public` and `public-history` are local only; never push.
- Release: bump version, tag, bump the overlay template (`./voidlab update voyage`).
- Backlog (dual boot, country autofill, doas, desktop choice, LUKS) is in
  `docs/continue-history-2026-10-08.md` at the workspace root.
