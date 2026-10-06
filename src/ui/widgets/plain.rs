use gtk::prelude::*;
use gtk::gio;

use super::{row_common, string_list};

pub type App = gtk::Application;
pub type Window = gtk::ApplicationWindow;

pub fn init() {}

pub fn new_app(id: &str) -> App {
    gtk::Application::builder().application_id(id).build()
}

pub fn new_window(app: &App, title: &str, width: i32, height: i32) -> Window {
    gtk::ApplicationWindow::builder().application(app).title(title).default_width(width).default_height(height).build()
}

pub fn set_window_content(window: &Window, header: &gtk::HeaderBar, body: &gtk::Widget) {
    window.set_titlebar(Some(header));
    window.set_child(Some(body));
}

fn root_window(widget: &impl IsA<gtk::Widget>) -> Option<gtk::Window> {
    widget.root().and_then(|root| root.downcast::<gtk::Window>().ok())
}

pub fn alert(widget: &impl IsA<gtk::Widget>, heading: &str, body: &str) {
    let Some(window) = root_window(widget) else { return };
    gtk::AlertDialog::builder()
        .message(heading)
        .detail(body)
        .buttons(["OK"])
        .default_button(0)
        .build()
        .choose(Some(&window), gio::Cancellable::NONE, |_| {});
}

/// Cancel / confirm dialog; `on_confirm` runs only when the confirm button is picked.
pub fn confirm(
    widget: &impl IsA<gtk::Widget>,
    heading: &str,
    body: &str,
    confirm_label: &str,
    on_confirm: impl FnOnce() + 'static,
) {
    let Some(window) = root_window(widget) else { return };
    gtk::AlertDialog::builder()
        .message(heading)
        .detail(body)
        .buttons(["Cancel", confirm_label])
        .cancel_button(0)
        .default_button(0)
        .build()
        .choose(Some(&window), gio::Cancellable::NONE, move |result| {
            if let Ok(1) = result {
                on_confirm();
            }
        });
}

pub fn about(parent: &Window) {
    gtk::AboutDialog::builder()
        .transient_for(parent)
        .modal(true)
        .program_name("Voyage")
        .logo_icon_name("system-software-install")
        .authors(["Void Dinit ISO"])
        .version("0.1.0 (GTK4)")
        .copyright("\u{a9} 2026 Void Dinit ISO")
        .license_type(gtk::License::Gpl30)
        .build()
        .present();
}

/// A titled, boxed list of rows.
#[derive(Clone)]
pub struct Group {
    root: gtk::Box,
    list: gtk::ListBox,
}

impl Group {
    pub fn new(title: &str) -> Self {
        let root = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(6).build();
        root.append(&gtk::Label::builder().label(title).halign(gtk::Align::Start).css_classes(["heading"]).build());
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["rich-list", "voyage-group"])
            .build();
        let frame = gtk::Frame::builder().child(&list).build();
        root.append(&frame);
        Self { root, list }
    }

    pub fn add(&self, widget: &impl AsRef<gtk::Widget>) {
        self.list.append(widget.as_ref());
    }

    pub fn remove(&self, widget: &impl AsRef<gtk::Widget>) {
        self.list.remove(widget.as_ref());
    }
}
row_common!(Group, root);

fn base_row(title: &str, child: Option<&gtk::Widget>, vertical: bool) -> (gtk::ListBoxRow, gtk::Label, gtk::Label, gtk::Box) {
    let outer = gtk::Box::builder()
        .orientation(if vertical { gtk::Orientation::Vertical } else { gtk::Orientation::Horizontal })
        .spacing(12)
        .margin_top(8)
        .margin_bottom(8)
        .margin_start(12)
        .margin_end(12)
        .build();
    let text = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(2).hexpand(true).valign(gtk::Align::Center).build();
    let title_label = gtk::Label::builder().label(title).halign(gtk::Align::Start).wrap(true).xalign(0.0).build();
    let subtitle = gtk::Label::builder()
        .halign(gtk::Align::Start)
        .wrap(true)
        .xalign(0.0)
        .visible(false)
        .css_classes(["dim-label", "caption"])
        .build();
    text.append(&title_label);
    text.append(&subtitle);
    outer.append(&text);
    if let Some(child) = child {
        outer.append(child);
    }
    let row = gtk::ListBoxRow::builder().activatable(false).selectable(false).child(&outer).build();
    (row, title_label, subtitle, outer)
}

fn set_subtitle_text(label: &gtk::Label, text: &str) {
    label.set_label(text);
    label.set_visible(!text.is_empty());
}

#[derive(Clone)]
pub struct ActionRow {
    row: gtk::ListBoxRow,
    outer: gtk::Box,
    subtitle: gtk::Label,
}

impl ActionRow {
    pub fn new(title: &str) -> Self {
        let (row, _t, subtitle, outer) = base_row(title, None, false);
        Self { row, outer, subtitle }
    }

    pub fn set_subtitle(&self, text: &str) {
        set_subtitle_text(&self.subtitle, text);
    }

    /// A row that only holds one widget (no title).
    pub fn with_child(child: &gtk::Widget) -> Self {
        let outer = gtk::Box::builder().margin_top(8).margin_bottom(8).margin_start(12).margin_end(12).build();
        outer.append(child);
        let row = gtk::ListBoxRow::builder().activatable(false).selectable(false).child(&outer).build();
        Self { row, outer, subtitle: gtk::Label::new(None) }
    }

    pub fn add_suffix(&self, widget: &gtk::Widget) {
        self.outer.append(widget);
    }
}
row_common!(ActionRow, row);

#[derive(Clone)]
pub struct ComboRow {
    row: gtk::ListBoxRow,
    drop: gtk::DropDown,
}

impl ComboRow {
    pub fn new(title: &str, items: &[String]) -> Self {
        let drop = gtk::DropDown::builder().model(&string_list(items)).valign(gtk::Align::Center).build();
        let (row, _t, _s, _o) = base_row(title, Some(drop.upcast_ref()), false);
        Self { row, drop }
    }

    pub fn set_items(&self, items: &[String]) {
        self.drop.set_model(Some(&string_list(items)));
    }

    pub fn selected(&self) -> u32 {
        match self.drop.selected() {
            gtk::INVALID_LIST_POSITION => 0,
            index => index,
        }
    }

    pub fn set_selected(&self, index: u32) {
        self.drop.set_selected(index);
    }

    pub fn connect_selected(&self, f: impl Fn(&ComboRow) + 'static) {
        let this = self.clone();
        self.drop.connect_selected_notify(move |_| f(&this));
    }
}
row_common!(ComboRow, row);

#[derive(Clone)]
pub struct SwitchRow {
    row: gtk::ListBoxRow,
    switch: gtk::Switch,
    subtitle: gtk::Label,
}

impl SwitchRow {
    pub fn new(title: &str) -> Self {
        let switch = gtk::Switch::builder().valign(gtk::Align::Center).build();
        let (row, _t, subtitle, _o) = base_row(title, Some(switch.upcast_ref()), false);
        Self { row, switch, subtitle }
    }

    pub fn set_subtitle(&self, text: &str) {
        set_subtitle_text(&self.subtitle, text);
    }

    pub fn is_active(&self) -> bool {
        self.switch.is_active()
    }

    pub fn set_active(&self, active: bool) {
        self.switch.set_active(active);
    }

    pub fn connect_active(&self, f: impl Fn(&SwitchRow) + 'static) {
        let this = self.clone();
        self.switch.connect_active_notify(move |_| f(&this));
    }
}
row_common!(SwitchRow, row);

#[derive(Clone)]
pub struct EntryRow {
    row: gtk::ListBoxRow,
    entry: gtk::Editable,
}

impl EntryRow {
    fn build(title: &str, entry: gtk::Widget) -> Self {
        let editable = entry.clone().dynamic_cast::<gtk::Editable>().expect("entries are editable");
        entry.set_hexpand(true);
        let (row, _t, _s, outer) = base_row(title, None, true);
        outer.append(&entry);
        Self { row, entry: editable }
    }

    pub fn new(title: &str, text: &str) -> Self {
        let entry = gtk::Entry::builder().text(text).build();
        Self::build(title, entry.upcast())
    }

    pub fn password(title: &str) -> Self {
        let entry = gtk::PasswordEntry::builder().show_peek_icon(true).build();
        Self::build(title, entry.upcast())
    }

    pub fn text(&self) -> String {
        self.entry.text().to_string()
    }

    pub fn set_text(&self, text: &str) {
        self.entry.set_text(text);
    }
}
row_common!(EntryRow, row);

#[derive(Clone)]
pub struct Banner {
    root: gtk::Revealer,
}

impl Banner {
    pub fn new(text: &str) -> Self {
        let label = gtk::Label::builder().label(text).wrap(true).margin_top(8).margin_bottom(8).margin_start(12).margin_end(12).build();
        let frame = gtk::Frame::builder().css_classes(["voyage-banner"]).child(&label).build();
        let root = gtk::Revealer::builder().reveal_child(true).child(&frame).build();
        Self { root }
    }
}
row_common!(Banner, root);
