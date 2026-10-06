use gtk::prelude::*;

const LOG_PATH: &str = "/tmp/installation.log";

pub const TITLE: &str = "Finished";

pub struct CompletionView {
    pub heading: &'static str,
    pub css_class: &'static str,
    pub icon_name: &'static str,
}

pub fn completion_view(success: bool) -> CompletionView {
    if success {
        CompletionView {
            heading: "Installation complete",
            css_class: "success",
            icon_name: "emblem-ok-symbolic",
        }
    } else {
        CompletionView {
            heading: "Installation failed",
            css_class: "error",
            icon_name: "dialog-error-symbolic",
        }
    }
}

pub struct CompletionPage {
    pub widget: gtk::Box,
    icon: gtk::Image,
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

        let icon = gtk::Image::builder().pixel_size(64).build();
        widget.append(&icon);

        let heading = gtk::Label::builder()
            .label("Installation complete")
            .css_classes(["title-1"])
            .build();
        widget.append(&heading);

        let detail = gtk::Label::builder()
            .label("Your system has been installed successfully.\nClick 'Restart' to complete the process.")
            .css_classes(["dim-label"])
            .build();
        widget.append(&detail);

        let log_button = gtk::Button::builder()
            .label("View Log")
            .halign(gtk::Align::Center)
            .visible(false)
            .build();
        widget.append(&log_button);

        let page_widget = widget.clone();
        log_button.connect_clicked(move |_| {
            let content = crate::backend::text::read_text_lossy(std::path::Path::new(LOG_PATH))
                .unwrap_or_else(|e| format!("Could not read {LOG_PATH}: {e}"));

            let text_view = gtk::TextView::builder()
                .editable(false)
                .monospace(true)
                .build();
            text_view.buffer().set_text(&content);
            let scroller = gtk::ScrolledWindow::builder()
                .vexpand(true)
                .hexpand(true)
                .build();
            scroller.set_child(Some(&text_view));

            let mut dialog = gtk::Window::builder()
                .title("Installation Log")
                .default_width(800)
                .default_height(600)
                .modal(true);
            if let Some(parent) = page_widget
                .root()
                .and_then(|root| root.downcast::<gtk::Window>().ok())
            {
                dialog = dialog.transient_for(&parent);
            }
            dialog.child(&scroller).build().present();
        });

        Self {
            widget,
            icon,
            heading,
            detail,
            log_button,
        }
    }

    pub fn set_result(&self, success: bool, message: &str) {
        let view = completion_view(success);
        self.icon.set_icon_name(Some(view.icon_name));
        self.heading.set_text(view.heading);
        self.heading.remove_css_class("success");
        self.heading.remove_css_class("error");
        self.heading.add_css_class(view.css_class);

        if success {
            self.detail.set_text("Your system has been installed successfully.\nClick 'Restart' to complete the process.");
        } else {
            let text = if message.is_empty() {
                format!("Check {LOG_PATH} for details.")
            } else {
                message.to_string()
            };
            self.detail.set_text(&text);
        }
        self.log_button.set_visible(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completion_view_distinguishes_success_and_failure() {
        let ok = completion_view(true);
        let fail = completion_view(false);
        assert_ne!(ok.css_class, fail.css_class);
        assert_ne!(ok.icon_name, fail.icon_name);
        assert_ne!(ok.heading, fail.heading);
    }

    #[test]
    fn completion_view_failure_uses_error_styling() {
        assert_eq!(completion_view(false).css_class, "error");
    }
}
