pub mod pages;
pub mod window;

#[derive(Debug, Clone, Default)]
pub struct SysData {
    pub efi: bool,
    pub net: bool,
    pub display_manager: Option<String>,
}
