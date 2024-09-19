#[cfg(debug_assertions)]
pub use self::debug_action::DebugAction;
pub use self::global_action::GlobalAction;
pub use self::unit_action::UnitAction;

#[cfg(debug_assertions)]
mod debug_action;
mod global_action;
mod unit_action;
