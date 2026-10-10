use gtk::glib;
use gtk::prelude::*;

use crate::backend::config_schema::InstallConfig;
use crate::backend::install_runner::{InstallEvent, InstallRunner};

pub const TITLE: &str = "Installing";

fn status_label(token: &str) -> &str {
    match token {
        "INIT" => "Starting installation engine...",
        "CREATE_FS" => "Creating file systems...",
        "COPY" => "Copying the base system...",
        "REGIONAL_CONFIG" => "Setting up language and time zone...",
        "UPDATE_DOWNLOAD" => "Downloading updates...",
        "UPDATE_INSTALL" => "Installing updates...",
        "MIRROR" => "Configuring repository server...",
        "NON-FREE" => "Configuring proprietary software repositories.",
        "HARDWARE" => "Installing drivers for this hardware...",
        "USER_CONFIG" => "Creating users and passwords...",
        "GRUB_INSTALL" => "Installing the boot loader...",
        "REMOVE_PACKAGES" => "Removing selected packages...",
        "DONE" => "Installation completed successfully",
        other => other,
    }
}

pub struct InstallationPage {
    pub widget: gtk::Box,
    status_label: gtk::Label,
    progress_bar: gtk::ProgressBar,
    log_view: gtk::TextView,
}

impl InstallationPage {
    pub fn new() -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .margin_top(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let status_label = gtk::Label::builder()
            .label("Preparing to install...")
            .css_classes(["title-4"])
            .build();
        widget.append(&status_label);

        let progress_bar = gtk::ProgressBar::builder().show_text(true).build();
        widget.append(&progress_bar);

        let log_scroller = gtk::ScrolledWindow::builder().vexpand(true).build();
        let log_view = gtk::TextView::builder()
            .editable(false)
            .monospace(true)
            .build();
        log_scroller.set_child(Some(&log_view));
        widget.append(&log_scroller);

        Self {
            widget,
            status_label,
            progress_bar,
            log_view,
        }
    }

    pub fn set_result(&self, success: bool, message: &str) {
        let view = super::completion::completion_view(success);
        self.status_label.set_text(if success {
            "Installation complete. Click Restart to start your installed system."
        } else {
            view.heading
        });
        self.status_label.remove_css_class("success");
        self.status_label.remove_css_class("error");
        self.status_label.add_css_class(view.css_class);
        if success {
            self.progress_bar.set_fraction(1.0);
            self.progress_bar.set_text(Some("100%"));
        } else if !message.is_empty() {
            let buffer = self.log_view.buffer();
            let mut end = buffer.end_iter();
            buffer.insert(&mut end, &format!("\nInstallation failed: {message}\n"));
        }
    }

    pub fn start<F>(&self, config: InstallConfig, demo: bool, on_finished: F)
    where
        F: Fn(bool, String) + 'static,
    {
        self.status_label.set_text(if demo {
            "Starting simulation (demo mode)..."
        } else {
            "Starting installation engine (Root)..."
        });

        let (tx, rx) = async_channel::unbounded::<InstallEvent>();
        let runner = InstallRunner::new(config, demo);
        runner.start(move |event| {
            let _ = tx.send_blocking(event);
        });

        let status_label = self.status_label.clone();
        let progress_bar = self.progress_bar.clone();
        let log_view = self.log_view.clone();
        glib::spawn_future_local(async move {
            while let Ok(event) = rx.recv().await {
                match event {
                    InstallEvent::Progress(value) => {
                        progress_bar.set_fraction(value as f64 / 100.0);
                        progress_bar.set_text(Some(&format!("{value}%")));
                    }
                    InstallEvent::Status(token) => {
                        status_label.set_text(status_label_owned(&token).as_str());
                    }
                    InstallEvent::Log(line) => {
                        let buf = log_view.buffer();
                        let mut end = buf.end_iter();
                        buf.insert(&mut end, &format!("{line}\n"));
                    }
                    InstallEvent::Success => {
                        on_finished(true, String::new());
                    }
                    InstallEvent::Error(message) => {
                        on_finished(false, message);
                    }
                }
            }
        });
    }
}

fn status_label_owned(token: &str) -> String {
    status_label(token).to_string()
}
