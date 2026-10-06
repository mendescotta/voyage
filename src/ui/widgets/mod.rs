//! One widget API with two backends, so the pages never name `adw::` directly:
//! plain GTK4 by default, libadwaita rows with `--features adwaita`.

#[cfg(feature = "adwaita")]
mod adw_impl;
#[cfg(not(feature = "adwaita"))]
mod plain;

#[cfg(feature = "adwaita")]
pub use adw_impl::*;
#[cfg(not(feature = "adwaita"))]
pub use plain::*;

/// Shared by both backends: `Clone`, `AsRef<gtk::Widget>` and visibility/sensitivity forwarding.
macro_rules! row_common {
    ($ty:ty, $field:tt) => {
        impl AsRef<gtk::Widget> for $ty {
            fn as_ref(&self) -> &gtk::Widget {
                self.$field.upcast_ref()
            }
        }
        #[allow(dead_code)]
        impl $ty {
            pub fn set_visible(&self, visible: bool) {
                self.$field.set_visible(visible);
            }
            pub fn set_sensitive(&self, sensitive: bool) {
                self.$field.set_sensitive(sensitive);
            }
        }
    };
}
pub(crate) use row_common;

pub fn string_list(items: &[String]) -> gtk::StringList {
    let model = gtk::StringList::new(&[]);
    for item in items {
        model.append(item);
    }
    model
}
