//! Shared Vizia widgets and theme tokens for the nxe plugins.
//!
//! This crate knows nothing about nih-plug. Widgets take a value and a
//! callback; binding them to plugin parameters is each plugin's job. See
//! `docs/specifications/architecture.md` for why that boundary exists.
//!
//! The widgets land in `UI-4` onward
//! (`docs/implementation/nxe-ui-plan.md`); the theme is here.

pub mod bar;
pub mod icon;
pub mod input;
pub mod knob;
pub mod theme;
