#[cfg(debug_assertions)]
pub use self::debug_action::DebugAction;
pub use self::global_action::GlobalAction;

#[cfg(debug_assertions)]
mod debug_action;
mod global_action;
