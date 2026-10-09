use adw::prelude::*;

use super::{row_common, string_list};

pub type App = adw::Application;
pub type Window = adw::ApplicationWindow;

pub fn init() {
    adw::init().expect("libadwaita init failed");
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
}

pub fn new_app(id: &str) -> App {
    adw::Application::builder().application_id(id).build()
}

pub fn new_window(app: &App, title: &str, width: i32, height: i32) -> Window {
    adw::ApplicationWindow::builder()
        .application(app)
        .title(title)
        .default_width(width)
        .default_height(height)
        .build()
}

pub fn set_window_content(window: &Window, header: &gtk::HeaderBar, body: &gtk::Widget) {
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(header);
    toolbar.set_content(Some(body));
    window.set_content(Some(&toolbar));
}

pub fn alert(widget: &impl IsA<gtk::Widget>, heading: &str, body: &str) {
    let dialog = adw::AlertDialog::builder()
        .heading(heading)
        .body(body)
        .build();
    dialog.add_response("ok", "OK");
    dialog.present(Some(widget));
}

/// Cancel / confirm dialog; `on_confirm` runs only when the confirm button is picked.
pub fn confirm(
    widget: &impl IsA<gtk::Widget>,
    heading: &str,
    body: &str,
    confirm_label: &str,
    on_confirm: impl FnOnce() + 'static,
) {
    let dialog = adw::AlertDialog::builder()
        .heading(heading)
        .body(body)
        .build();
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("continue", confirm_label);
    dialog.set_response_appearance("continue", adw::ResponseAppearance::Destructive);
    let on_confirm = std::cell::RefCell::new(Some(on_confirm));
    dialog.connect_response(None, move |_dialog, response| {
        if response == "continue" {
            if let Some(f) = on_confirm.borrow_mut().take() {
                f();
            }
        }
    });
    dialog.present(Some(widget));
}

pub fn about(parent: &Window) {
    let about = adw::AboutDialog::builder()
        .application_name("Voyage")
        .application_icon("system-software-install")
        .developer_name("Void Dinit ISO")
        .version(concat!(env!("CARGO_PKG_VERSION"), " (GTK4)"))
        .copyright("\u{a9} 2026 Void Dinit ISO")
        .license_type(gtk::License::Gpl30)
        .build();
    about.present(Some(parent));
}

#[derive(Clone)]
pub struct Group(adw::PreferencesGroup);

impl Group {
    pub fn new(title: &str) -> Self {
        Self(adw::PreferencesGroup::builder().title(title).build())
    }

    pub fn add(&self, widget: &impl AsRef<gtk::Widget>) {
        self.0.add(widget.as_ref());
    }

    pub fn remove(&self, widget: &impl AsRef<gtk::Widget>) {
        self.0.remove(widget.as_ref());
    }
}
row_common!(Group, 0);

#[derive(Clone)]
pub struct ActionRow(adw::ActionRow);

impl ActionRow {
    pub fn new(title: &str) -> Self {
        Self(adw::ActionRow::builder().title(title).build())
    }

    pub fn with_child(child: &gtk::Widget) -> Self {
        Self(adw::ActionRow::builder().child(child).build())
    }

    pub fn set_subtitle(&self, text: &str) {
        self.0.set_subtitle(text);
    }

    pub fn add_suffix(&self, widget: &gtk::Widget) {
        self.0.add_suffix(widget);
    }
}
row_common!(ActionRow, 0);

#[derive(Clone)]
pub struct ComboRow(adw::ComboRow);

impl ComboRow {
    pub fn new(title: &str, items: &[String]) -> Self {
        Self(
            adw::ComboRow::builder()
                .title(title)
                .model(&string_list(items))
                .build(),
        )
    }

    pub fn set_items(&self, items: &[String]) {
        self.0.set_model(Some(&string_list(items)));
    }

    pub fn selected(&self) -> u32 {
        match self.0.selected() {
            gtk::INVALID_LIST_POSITION => 0,
            index => index,
        }
    }

    pub fn set_selected(&self, index: u32) {
        self.0.set_selected(index);
    }

    pub fn connect_selected(&self, f: impl Fn(&ComboRow) + 'static) {
        let this = self.clone();
        self.0.connect_selected_notify(move |_| f(&this));
    }
}
row_common!(ComboRow, 0);

#[derive(Clone)]
pub struct SwitchRow(adw::SwitchRow);

impl SwitchRow {
    pub fn new(title: &str) -> Self {
        Self(adw::SwitchRow::builder().title(title).build())
    }

    pub fn set_subtitle(&self, text: &str) {
        self.0.set_subtitle(text);
    }

    pub fn is_active(&self) -> bool {
        self.0.is_active()
    }

    pub fn set_active(&self, active: bool) {
        self.0.set_active(active);
    }

    pub fn connect_active(&self, f: impl Fn(&SwitchRow) + 'static) {
        let this = self.clone();
        self.0.connect_active_notify(move |_| f(&this));
    }
}
row_common!(SwitchRow, 0);

#[derive(Clone)]
pub struct EntryRow {
    widget: gtk::Widget,
    entry: gtk::Editable,
}

impl EntryRow {
    pub fn new(title: &str, text: &str) -> Self {
        let row = adw::EntryRow::builder().title(title).text(text).build();
        Self {
            entry: row.clone().upcast(),
            widget: row.upcast(),
        }
    }

    pub fn password(title: &str) -> Self {
        let row = adw::PasswordEntryRow::builder().title(title).build();
        Self {
            entry: row.clone().upcast(),
            widget: row.upcast(),
        }
    }

    pub fn text(&self) -> String {
        self.entry.text().to_string()
    }

    pub fn set_text(&self, text: &str) {
        self.entry.set_text(text);
    }
}
row_common!(EntryRow, widget);

#[derive(Clone)]
pub struct Banner(adw::Banner);

impl Banner {
    pub fn new(text: &str) -> Self {
        Self(adw::Banner::builder().title(text).revealed(true).build())
    }
}
row_common!(Banner, 0);
