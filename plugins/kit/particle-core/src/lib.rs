//! The particle engines behind the Effloresce Kits
//! (`plugins/kit/docs/implementation/particle-kit-plan.md`): Glint first,
//! Pollen next. Effloresce takes these over when it comes.
//!
//! Host-agnostic and allocation-free once built.

pub mod glint;

pub use glint::Glint;
