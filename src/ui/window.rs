//! Port of `ui/window.py`'s `MainWindow`: wizard shell — sidebar step
//! list, page stack, Back/Next navigation, install orchestration.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;

use crate::backend::config_schema::{build_config, InstallFields};
use crate::ui::pages::completion::CompletionPage;
use crate::ui::pages::disks::DisksPage;
use crate::ui::pages::installation::InstallationPage;
use crate::ui::pages::mirrors::MirrorsPage;
use crate::ui::pages::review::ReviewPage;
use crate::ui::pages::users::UsersPage;
use crate::ui::pages::welcome::WelcomePage;
use crate::ui::{pages, SysData};

const PAGE_COUNT: usize = 7;
const WELCOME: usize = 0;
const MIRRORS: usize = 1;
const USERS: usize = 2;
const DISKS: usize = 3;
const REVIEW: usize = 4;
const INSTALLATION: usize = 5;
const COMPLETION: usize = 6;

struct State {
    sys_data: SysData,
    demo: bool,
    welcome: WelcomePage,
    mirrors: MirrorsPage,
    users: UsersPage,
    disks: DisksPage,
    review: ReviewPage,
    installation: InstallationPage,
    completion: CompletionPage,
    stack: gtk::Stack,
    step_rows: Vec<adw::ActionRow>,
    back_button: gtk::Button,
    next_button: gtk::Button,
    current_index: usize,
    window: adw::ApplicationWindow,
}

pub fn build(app: &adw::Application, sys_data: SysData, demo: bool) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Voyage")
        .default_width(880)
        .default_height(620)
        .build();

    let welcome = WelcomePage::new(&sys_data);
    let mirrors = MirrorsPage::new(&sys_data);
    let users = UsersPage::new(&sys_data);
    let disks = DisksPage::new(&sys_data);
    let review = ReviewPage::new();
    let installation = InstallationPage::new();
    let completion = CompletionPage::new();

    let stack = gtk::Stack::new();
    stack.add_titled(&welcome.widget, Some(pages::welcome::TITLE), pages::welcome::TITLE);
    stack.add_titled(&mirrors.widget, Some(pages::mirrors::TITLE), pages::mirrors::TITLE);
    stack.add_titled(&users.widget, Some(pages::users::TITLE), pages::users::TITLE);
    stack.add_titled(&disks.widget, Some(pages::disks::TITLE), pages::disks::TITLE);
    stack.add_titled(&review.widget, Some(pages::review::TITLE), pages::review::TITLE);
    stack.add_titled(&installation.widget, Some(pages::installation::TITLE), pages::installation::TITLE);
    stack.add_titled(&completion.widget, Some(pages::completion::TITLE), pages::completion::TITLE);

    let sidebar_list = gtk::ListBox::builder().selection_mode(gtk::SelectionMode::None).css_classes(["navigation-sidebar"]).build();
    let step_titles = [pages::welcome::TITLE, pages::mirrors::TITLE, pages::users::TITLE, pages::disks::TITLE];
    let mut step_rows = Vec::new();
    for title in step_titles {
        let row = adw::ActionRow::builder().title(title).activatable(true).build();
        sidebar_list.append(&row);
        step_rows.push(row);
    }

    let sidebar_toolbar = adw::ToolbarView::new();
    sidebar_toolbar.add_top_bar(&adw::HeaderBar::builder().show_end_title_buttons(false).build());
    sidebar_toolbar.set_content(Some(&sidebar_list));

    let header_bar = adw::HeaderBar::new();
    let about_button = gtk::Button::builder().icon_name("help-about-symbolic").tooltip_text("About").build();
    header_bar.pack_end(&about_button);

    let content_toolbar = adw::ToolbarView::new();
    content_toolbar.add_top_bar(&header_bar);
    content_toolbar.set_content(Some(&stack));

    let back_button = gtk::Button::builder().label("Back").build();
    let next_button = gtk::Button::builder().label("Next").css_classes(["suggested-action"]).build();

    let bottom_bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(6)
        .margin_top(6)
        .margin_bottom(6)
        .margin_start(12)
        .margin_end(12)
        .build();
    bottom_bar.append(&back_button);
    bottom_bar.append(&gtk::Box::builder().hexpand(true).build());
    bottom_bar.append(&next_button);
    content_toolbar.add_bottom_bar(&bottom_bar);

    let split_view = adw::NavigationSplitView::new();
    split_view.set_sidebar(Some(&adw::NavigationPage::builder().title("Steps").child(&sidebar_toolbar).build()));
    split_view.set_content(Some(&adw::NavigationPage::builder().title("Voyage").child(&content_toolbar).build()));

    window.set_content(Some(&split_view));

    let state = Rc::new(RefCell::new(State {
        sys_data,
        demo,
        welcome,
        mirrors,
        users,
        disks,
        review,
        installation,
        completion,
        stack,
        step_rows,
        back_button,
        next_button,
        current_index: 0,
        window: window.clone(),
    }));

    update_nav(&state);

    {
        let state = state.clone();
        sidebar_list.connect_row_activated(move |_listbox, row| {
            let mut s = state.borrow_mut();
            if s.current_index == INSTALLATION {
                return;
            }
            if let Some(index) = s.step_rows.iter().position(|r| r == row) {
                s.current_index = index;
            }
            drop(s);
            update_nav(&state);
        });
    }

    {
        let back_button = state.borrow().back_button.clone();
        let state = state.clone();
        back_button.connect_clicked(move |_| {
            let mut s = state.borrow_mut();
            if s.current_index > 0 {
                s.current_index -= 1;
            }
            drop(s);
            update_nav(&state);
        });
    }

    {
        let next_button = state.borrow().next_button.clone();
        let state = state.clone();
        next_button.connect_clicked(move |_| on_next(&state));
    }

    {
        let state = state.clone();
        about_button.connect_clicked(move |_| show_about(&state));
    }

    {
        let state = state.clone();
        let install_button = state.borrow().review.install_button.clone();
        install_button.connect_clicked(move |_| start_install(&state));
    }

    window
}

fn update_nav(state: &Rc<RefCell<State>>) {
    let s = state.borrow();
    let page = s.current_index;
    let visible_title = match page {
        WELCOME => pages::welcome::TITLE,
        MIRRORS => pages::mirrors::TITLE,
        USERS => pages::users::TITLE,
        DISKS => pages::disks::TITLE,
        REVIEW => pages::review::TITLE,
        INSTALLATION => pages::installation::TITLE,
        _ => pages::completion::TITLE,
    };
    s.stack.set_visible_child_name(visible_title);

    let is_install_step = page == INSTALLATION;
    let is_review_step = page == REVIEW;
    s.back_button.set_sensitive(page > 0 && page < PAGE_COUNT - 1 && !is_install_step);

    let is_last = page == PAGE_COUNT - 1;
    s.next_button.set_visible(!is_install_step && !is_review_step);
    s.next_button.set_label(if is_last { "Restart" } else { "Next" });

    for (i, row) in s.step_rows.iter().enumerate() {
        row.remove_css_class("accent");
        if i == page {
            row.add_css_class("accent");
        }
    }
}

fn on_next(state: &Rc<RefCell<State>>) {
    let current = state.borrow().current_index;

    if current == DISKS {
        match collect_all(state) {
            Ok(config) => {
                state.borrow().review.set_config(&config);
                state.borrow_mut().current_index = REVIEW;
                update_nav(state);
            }
            Err(errors) => show_errors(state, &errors),
        }
        return;
    }
    if current == COMPLETION {
        let (demo, app) = {
            let s = state.borrow();
            (s.demo, s.window.application())
        };
        if demo {
            if let Some(app) = app {
                app.quit();
            }
        } else {
            let _ = std::process::Command::new("pkexec").arg("reboot").spawn();
        }
        return;
    }

    let errors = match current {
        WELCOME => state.borrow().welcome.collect().1,
        MIRRORS => state.borrow().mirrors.collect().1,
        USERS => state.borrow().users.collect().1,
        _ => Vec::new(),
    };
    if !errors.is_empty() {
        show_errors(state, &errors);
        return;
    }

    state.borrow_mut().current_index += 1;
    update_nav(state);
}

fn collect_all(state: &Rc<RefCell<State>>) -> Result<crate::backend::config_schema::InstallConfig, Vec<(String, String)>> {
    let s = state.borrow();
    let (welcome_fields, mut errors) = s.welcome.collect();
    let (mirrors_fields, mirrors_errors) = s.mirrors.collect();
    let (users_fields, users_errors) = s.users.collect();
    errors.extend(mirrors_errors);
    errors.extend(users_errors);

    let (disk_choices, disks_errors) = s.disks.collect();
    errors.extend(disks_errors);
    if !errors.is_empty() {
        return Err(errors);
    }

    let fields = InstallFields {
        locale: welcome_fields.locale,
        timezone_region: welcome_fields.timezone_region,
        timezone_city: welcome_fields.timezone_city,
        keymap: welcome_fields.keymap,
        hostname: users_fields.hostname,
        userlogin: users_fields.userlogin,
        username: users_fields.username,
        userpassword: users_fields.userpassword,
        rootpassword: users_fields.rootpassword,
        autologin: users_fields.autologin,
        mirror: mirrors_fields.mirror,
        net: mirrors_fields.net,
        nonfree: mirrors_fields.nonfree,
        nvidia: mirrors_fields.nvidia,
        intel: mirrors_fields.intel,
    };

    build_config(&fields, &disk_choices, s.sys_data.display_manager.as_deref().unwrap_or(""))
}

fn start_install(state: &Rc<RefCell<State>>) {
    let data = match collect_all(state) {
        Ok(data) => data,
        Err(errors) => {
            show_errors(state, &errors);
            return;
        }
    };

    {
        let mut s = state.borrow_mut();
        s.current_index = INSTALLATION;
    }
    update_nav(state);

    let demo = state.borrow().demo;
    let state_for_finish = state.clone();
    state.borrow().installation.start(data, demo, move |success, message| {
        {
            let s = state_for_finish.borrow();
            s.completion.set_result(success, &message);
        }
        state_for_finish.borrow_mut().current_index = COMPLETION;
        update_nav(&state_for_finish);
    });
}

fn show_errors(state: &Rc<RefCell<State>>, errors: &[(String, String)]) {
    let body = errors.iter().map(|(_, message)| message.as_str()).collect::<Vec<_>>().join("\n");
    let dialog = adw::AlertDialog::builder().heading("Please check your input").body(body).build();
    dialog.add_response("ok", "OK");
    let s = state.borrow();
    dialog.present(Some(&s.window));
}

fn show_about(state: &Rc<RefCell<State>>) {
    let about = adw::AboutDialog::builder()
        .application_name("Voyage")
        .application_icon("system-software-install")
        .developer_name("Void Dinit ISO")
        .version("0.1.0 (GTK4)")
        .copyright("\u{a9} 2026 Void Dinit ISO")
        .license_type(gtk::License::Gpl30)
        .build();
    let s = state.borrow();
    about.present(Some(&s.window));
}
