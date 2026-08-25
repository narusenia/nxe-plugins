//! Shared Vizia widgets and theme tokens for the nxe plugins.
//!
//! This crate knows nothing about nih-plug. Widgets take a value and a
//! callback; binding them to plugin parameters is each plugin's job. See
//! `docs/specifications/architecture.md` for why that boundary exists.
//!
//! The widgets themselves land in `UI-1` through `UI-9`
//! (`docs/implementation/nxe-ui-plan.md`). Right now this crate exists so the
//! workspace and the standalone gallery are wired up.
