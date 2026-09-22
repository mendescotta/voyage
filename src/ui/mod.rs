pub mod pages;
pub mod window;

/// System state detected once at startup, threaded through to every page —
/// port of the `sys_data` dict `main.py`/`window.py` pass around.
#[derive(Debug, Clone, Default)]
pub struct SysData {
    pub efi: bool,
    pub net: bool,
    pub display_manager: Option<String>,
}
