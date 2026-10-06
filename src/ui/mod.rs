pub mod nav;
pub mod pages;
pub mod widgets;
pub mod window;

/// Compiled into the binary so the theme also loads on an installed system.
/// The plain GTK4 build follows the desktop theme; the libadwaita build adds its dark palette.
#[cfg(feature = "adwaita")]
pub const STYLE_CSS: &str = concat!(include_str!("style-adwaita.css"), "\n", include_str!("style.css"));
#[cfg(not(feature = "adwaita"))]
pub const STYLE_CSS: &str = include_str!("style.css");

#[derive(Debug, Clone, Default)]
pub struct SysData {
    pub efi: bool,
    pub net: bool,
    pub display_manager: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::STYLE_CSS;

    #[test]
    fn stylesheet_is_embedded_and_styles_the_tab_bar() {
        assert!(STYLE_CSS.contains(".voyage-tabs"));
    }

    #[cfg(feature = "adwaita")]
    #[test]
    fn adwaita_build_defines_the_dark_palette() {
        assert!(STYLE_CSS.contains("@define-color window_bg_color"));
        assert!(STYLE_CSS.contains("@define-color accent_bg_color"));
    }

    #[cfg(not(feature = "adwaita"))]
    #[test]
    fn plain_build_leaves_colors_to_the_gtk_theme() {
        assert!(!STYLE_CSS.contains("@define-color"));
    }

    #[test]
    fn main_does_not_load_the_stylesheet_from_the_build_directory() {
        let main_rs = include_str!("../main.rs");
        assert!(
            !main_rs.contains("CARGO_MANIFEST_DIR"),
            "main.rs must not read style.css from a build-time path: it does not exist on an installed system"
        );
    }
}
