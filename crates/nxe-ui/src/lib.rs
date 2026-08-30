//! Shared Vizia widgets and theme tokens for the nxe plugins.
//!
//! This crate knows nothing about nih-plug. Widgets take a value and a
//! callback; binding them to plugin parameters is each plugin's job. See
//! `docs/specifications/architecture.md` for why that boundary exists.
//!
//! The widgets land in `UI-4` onward
//! (`docs/implementation/nxe-ui-plan.md`); the theme is here.

pub mod band;
pub mod bar;
pub mod curve;
pub mod dots;
pub mod entry;
pub mod font;
pub mod header;
pub mod heartbeat;
pub mod icon;
pub mod input;
pub mod knob;
pub mod meter;
pub mod polar;
pub mod readout;
pub mod segmented;
pub mod surface;
pub mod taps;
pub mod theme;
