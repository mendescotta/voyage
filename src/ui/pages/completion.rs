//! Port of `ui/pages/completion.py`: final success/failure screen with a
//! "View Log" dialog.

use std::fs;

use adw::prelude::*;

const LOG_PATH: &str = "/tmp/installation.log";

pub const TITLE: &str = "Finished";

pub struct CompletionPage {
    pub widget: gtk::Box,
    heading: gtk::Label,
    detail: gtk::Label,
    log_button: gtk::Button,
}

impl CompletionPage {
    pub fn new() -> Self {
        let widget = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .valign(gtk::Align::Center)
            .margin_top(48)
            .build();

        let heading = gtk::Label::builder().label("Installation complete").css_classes(["title-1"]).build();
        widget.append(&heading);

        let detail = gtk::Label::builder()
            .label("Your system has been installed successfully.\nClick 'Restart' to complete the process.")
            .css_classes(["dim-label"])
            .build();
        widget.append(&detail);

        let log_button = gtk::Button::builder().label("View Log").halign(gtk::Align::Center).visible(false).build();
        widget.append(&log_button);

        let page_widget = widget.clone();
        log_button.connect_clicked(move |_| {
            let content = fs::read_to_string(LOG_PATH)
                .unwrap_or_else(|e| format!("Could not read {LOG_PATH}: {e}"));

            let text_view = gtk::TextView::builder().editable(false).monospace(true).build();
            text_view.buffer().set_text(&content);
            let scroller = gtk::ScrolledWindow::builder().vexpand(true).hexpand(true).build();
            scroller.set_child(Some(&text_view));

            let toolbar_view = adw::ToolbarView::new();
            toolbar_view.add_top_bar(&adw::HeaderBar::new());
            toolbar_view.set_content(Some(&scroller));

            let dialog = adw::Dialog::builder().title("Installation Log").content_width(800).content_height(600).build();
            dialog.set_child(Some(&toolbar_view));
            if let Some(root) = page_widget.root() {
                dialog.present(Some(&root));
            }
        });

        Self { widget, heading, detail, log_button }
    }

    pub fn set_result(&self, success: bool, message: &str) {
        if success {
            self.heading.set_text("Installation complete");
            self.detail.set_text("Your system has been installed successfully.\nClick 'Restart' to complete the process.");
        } else {
            self.heading.set_text("Installation failed");
            let text = if message.is_empty() { format!("Check {LOG_PATH} for details.") } else { message.to_string() };
            self.detail.set_text(&text);
        }
        self.log_button.set_visible(true);
    }
}
