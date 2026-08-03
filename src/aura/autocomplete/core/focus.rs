//! Accessibility focus, clipboard/paste insertion, and key state probes.
//!
//! Delegates to the shared `accessibility` middleware module.

pub(super) use crate::aura::accessibility::any_modifier_down;
pub(super) use crate::aura::accessibility::apply_text_to_focused_field;
pub(super) use crate::aura::accessibility::focused_text_context_verbose;
pub(super) use crate::aura::accessibility::is_escape_key_down;
pub(super) use crate::aura::accessibility::is_tab_key_down;
pub(super) use crate::aura::accessibility::send_backspace;
#[cfg(target_os = "macos")]
pub(super) use crate::aura::accessibility::validate_focused_target;
