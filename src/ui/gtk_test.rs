//! Test support: run GTK code on one dedicated thread that initialises GTK exactly once.
//! GTK is not thread-safe, and a failed `gtk::init()` (no display, as on CI) must not be repeated: several
//! of them in one process corrupt GTK's resource registration and crash the test binary at exit.

use std::sync::{mpsc, Mutex, OnceLock};

type Job = Box<dyn FnOnce() + Send>;

static RUNNER: OnceLock<Option<Mutex<mpsc::Sender<Job>>>> = OnceLock::new();

/// Runs `f` on the GTK thread and re-raises its panic here. Without a display it prints a note and skips.
pub fn run<F: FnOnce() + Send + 'static>(what: &str, f: F) {
    let runner = RUNNER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        let (ready_tx, ready_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let ok = gtk::init().is_ok();
            let _ = ready_tx.send(ok);
            if ok {
                for job in rx {
                    job();
                }
            }
        });
        ready_rx.recv().unwrap_or(false).then(|| Mutex::new(tx))
    });
    let Some(tx) = runner else {
        eprintln!("no display: skipping the GTK part of {what}");
        return;
    };
    let (done_tx, done_rx) = mpsc::channel();
    let job: Job = Box::new(move || {
        let _ = done_tx.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)));
    });
    tx.lock()
        .unwrap()
        .send(job)
        .expect("the GTK thread is gone");
    if let Err(panic) = done_rx.recv().expect("the GTK thread died") {
        std::panic::resume_unwind(panic);
    }
}
