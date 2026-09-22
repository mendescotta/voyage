//! `InstallRunner`: writes the conf
//! file, spawns `pkexec bash backend_install.sh`, and turns its `>>> TOKEN`
//! stdout lines into progress/status/log events — or replays a scripted
//! demo sequence when `demo` is set.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::config_schema::InstallConfig;
use super::paths::backend_install_script;

#[derive(Debug, Clone)]
pub enum InstallEvent {
    Progress(u8),
    Status(String),
    Log(String),
    Success,
    Error(String),
}

const PROGRESS_MILESTONES: &[(&str, u8)] = &[
    ("INIT", 5),
    ("CREATE_FS", 15),
    ("REGIONAL_CONFIG", 50),
    ("USER_CONFIG", 85),
    ("GRUB_INSTALL", 90),
    ("DONE", 100),
];

const PROGRESS_RAMPS: &[(&str, (u8, u8))] = &[
    ("COPY", (30, 49)),
    ("UPDATE_DOWNLOAD", (50, 69)),
    ("UPDATE_INSTALL", (60, 69)),
];

const DEMO_STEPS: &[(&str, u8, u64)] = &[
    ("INIT", 5, 400),
    ("CREATE_FS", 15, 400),
    ("COPY", 45, 1200),
    ("REGIONAL_CONFIG", 55, 400),
    ("UPDATE_DOWNLOAD", 60, 1000),
    ("UPDATE_INSTALL", 70, 1000),
    ("USER_CONFIG", 85, 400),
    ("GRUB_INSTALL", 95, 400),
    ("DONE", 100, 200),
];

fn escape_conf_value(value: &str) -> String {
    // backend_install.sh reads `KEY value` lines with a plain grep/sed, and
    // values here (paths, hostnames, passwords) are not expected to contain
    // newlines; we only guard against a literal newline breaking the
    // line-oriented format.
    value.replace('\n', " ")
}

/// Writes `/tmp/.void-installer.conf` in the `KEY value` / `MOUNTPOINT ...`
/// format `backend_install.sh` expects, mode 0600.
pub fn generate_conf_file(config: &InstallConfig, conf_file: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(conf_file)?;

    let lines: Vec<(&str, String)> = vec![
        ("LOCALE", config.locale.clone()),
        ("TIMEZONE", config.timezone.clone()),
        ("KEYMAP", config.keymap.clone()),
        ("HOSTNAME", config.hostname.clone()),
        ("USERLOGIN", config.userlogin.clone()),
        ("USERNAME", config.username.clone()),
        ("USERPASSWORD", config.userpassword.clone()),
        ("ROOTPASSWORD", config.rootpassword.clone()),
        ("USERGROUPS", config.usergroups.clone()),
        ("DISPLAYMANAGER", config.display_manager.clone()),
        ("AUTOLOGIN", if config.autologin { "1" } else { "0" }.to_string()),
        ("MIRROR", config.mirror.clone()),
        ("UPDATE", if config.update { "1" } else { "0" }.to_string()),
        ("NONFREE", if config.nonfree { "1" } else { "0" }.to_string()),
        ("NVIDIA", if config.nvidia { "1" } else { "0" }.to_string()),
        ("INTEL", if config.intel { "1" } else { "0" }.to_string()),
        ("BOOTLOADER", config.bootloader_disk.clone()),
        ("BOOTLOADER_TYPE", config.bootloader_type.clone()),
    ];
    for (key, value) in lines {
        writeln!(file, "{key} {}", escape_conf_value(&value))?;
    }

    for part in &config.partitions {
        // MOUNTPOINT dev fs size point format — "size" is unused by
        // backend_install.sh's actual partitioning (kept as "0G" the way
        // install_runner.py always did) and format is "1"/"0".
        writeln!(
            file,
            "MOUNTPOINT {} {} 0G {} {}",
            part.dev,
            part.fs,
            part.point,
            if part.format { 1 } else { 0 },
        )?;
    }

    Ok(())
}

pub struct InstallRunner {
    config: InstallConfig,
    demo: bool,
    conf_file: String,
    interrupted: Arc<AtomicBool>,
}

impl InstallRunner {
    pub fn new(config: InstallConfig, demo: bool) -> Self {
        Self {
            config,
            demo,
            conf_file: "/tmp/.void-installer.conf".to_string(),
            interrupted: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Not wired to any UI control yet — `install_runner.py` exposes the
    /// same method unused by `window.py` today; kept for parity and future
    /// cancel-button support.
    #[allow(dead_code)]
    pub fn request_interruption(&self) {
        self.interrupted.store(true, Ordering::SeqCst);
    }

    /// Spawns a background thread that runs the install (or demo
    /// simulation) and sends `InstallEvent`s to `on_event`. `on_event` is
    /// called from the background thread — callers are expected to
    /// marshal it back to the GTK main loop themselves (e.g. via a
    /// `glib::MainContext` channel), since this module has no GTK
    /// dependency.
    pub fn start<F>(self, on_event: F) -> thread::JoinHandle<()>
    where
        F: Fn(InstallEvent) + Send + Sync + 'static,
    {
        let on_event = Arc::new(on_event);
        thread::spawn(move || {
            if self.demo {
                self.run_demo(on_event.as_ref());
            } else {
                self.run_real(&on_event);
            }
        })
    }

    fn run_demo<F: Fn(InstallEvent)>(&self, on_event: &F) {
        on_event(InstallEvent::Log(
            "[DEMO] Demo mode active: no real changes will be made.".to_string(),
        ));
        for (token, value, delay_ms) in DEMO_STEPS {
            if self.interrupted.load(Ordering::SeqCst) {
                return;
            }
            on_event(InstallEvent::Status((*token).to_string()));
            on_event(InstallEvent::Log(format!("[DEMO] Simulating step: {token}")));
            on_event(InstallEvent::Progress(*value));
            thread::sleep(Duration::from_millis(*delay_ms));
        }
        on_event(InstallEvent::Success);
    }

    fn run_real<F: Fn(InstallEvent) + Send + Sync + 'static>(&self, on_event: &Arc<F>) {
        on_event(InstallEvent::Status("INIT".to_string()));
        if let Err(e) = generate_conf_file(&self.config, &self.conf_file) {
            on_event(InstallEvent::Error(format!("Error writing the configuration: {e}")));
            return;
        }

        let script = backend_install_script();
        let mut child = match Command::new("pkexec")
            .arg("bash")
            .arg(&script)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                on_event(InstallEvent::Error(format!("Critical error running the backend: {e}")));
                return;
            }
        };

        let Some(stdout) = child.stdout.take() else {
            on_event(InstallEvent::Error("Critical error running the backend: no stdout".to_string()));
            return;
        };
        let reader = BufReader::new(stdout);

        let mut ramp_stop: Option<Arc<AtomicBool>> = None;
        let mut ramp_handle: Option<thread::JoinHandle<()>> = None;
        let stop_ramp = |ramp_stop: &mut Option<Arc<AtomicBool>>, ramp_handle: &mut Option<thread::JoinHandle<()>>| {
            if let Some(flag) = ramp_stop.take() {
                flag.store(true, Ordering::SeqCst);
            }
            if let Some(handle) = ramp_handle.take() {
                let _ = handle.join();
            }
        };

        for line in reader.lines() {
            let Ok(line) = line else { break };
            let clean_line = line.trim();
            if !clean_line.starts_with(">>>") {
                on_event(InstallEvent::Log(clean_line.to_string()));
                continue;
            }

            let msg = clean_line.trim_start_matches(">>>").trim().to_string();
            on_event(InstallEvent::Status(msg.clone()));
            on_event(InstallEvent::Log(format!("[INFO] {msg}")));

            if let Some((_, range)) = PROGRESS_RAMPS.iter().find(|(token, _)| msg.contains(token)) {
                stop_ramp(&mut ramp_stop, &mut ramp_handle);
                let flag = Arc::new(AtomicBool::new(false));
                let flag_clone = flag.clone();
                let (start, end) = *range;
                let on_event = Arc::clone(on_event);
                ramp_handle = Some(thread::spawn(move || {
                    let mut current = start;
                    while !flag_clone.load(Ordering::SeqCst) && current < end {
                        current += 1;
                        on_event(InstallEvent::Progress(current));
                        thread::sleep(Duration::from_millis(10_000));
                    }
                }));
                ramp_stop = Some(flag);
                continue;
            }

            if let Some((_, value)) = PROGRESS_MILESTONES.iter().find(|(token, _)| msg.contains(token)) {
                stop_ramp(&mut ramp_stop, &mut ramp_handle);
                on_event(InstallEvent::Progress(*value));
            }
        }

        stop_ramp(&mut ramp_stop, &mut ramp_handle);

        match child.wait() {
            Ok(status) if status.success() => on_event(InstallEvent::Success),
            _ => on_event(InstallEvent::Error(
                "The installer has failed. Check /tmp/installation.log.".to_string(),
            )),
        }
    }
}

#[allow(dead_code)]
fn ensure_tmp_dir_exists() {
    let _ = fs::create_dir_all("/tmp");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::config_schema::Partition;

    fn sample_config() -> InstallConfig {
        InstallConfig {
            locale: "en_US.UTF-8".to_string(),
            timezone: "America/New_York".to_string(),
            keymap: "us".to_string(),
            hostname: "void-box".to_string(),
            userlogin: "gui".to_string(),
            username: "Gui".to_string(),
            userpassword: "hunter2".to_string(),
            rootpassword: "hunter2root".to_string(),
            usergroups: "wheel,audio,video,users,network,optical,cdrom".to_string(),
            display_manager: "gdm".to_string(),
            autologin: false,
            mirror: "Default".to_string(),
            update: true,
            nonfree: false,
            nvidia: false,
            intel: false,
            partitions: vec![
                Partition { dev: "/dev/sda1".to_string(), point: "/boot/efi".to_string(), fs: "vfat".to_string(), format: true },
                Partition { dev: "/dev/sda2".to_string(), point: "/".to_string(), fs: "ext4".to_string(), format: true },
            ],
            bootloader_disk: "/dev/sda".to_string(),
            bootloader_type: "grub".to_string(),
        }
    }

    #[test]
    fn conf_file_matches_backend_install_sh_format() {
        let config = sample_config();
        let path = format!("/tmp/voyage-test-conf-{}.conf", std::process::id());
        generate_conf_file(&config, &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        fs::remove_file(&path).ok();

        assert!(content.contains("HOSTNAME void-box\n"));
        assert!(content.contains("TIMEZONE America/New_York\n"));
        assert!(content.contains("MOUNTPOINT /dev/sda1 vfat 0G /boot/efi 1\n"));
        assert!(content.contains("MOUNTPOINT /dev/sda2 ext4 0G / 1\n"));
        assert!(content.contains("UPDATE 1\n"));
        assert!(content.contains("BOOTLOADER /dev/sda\n"));
        assert!(content.contains("BOOTLOADER_TYPE grub\n"));
    }

    #[test]
    fn conf_file_is_written_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let config = sample_config();
        let path = format!("/tmp/voyage-test-conf-perm-{}.conf", std::process::id());
        generate_conf_file(&config, &path).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        fs::remove_file(&path).ok();
        assert_eq!(mode, 0o600);
    }
}
