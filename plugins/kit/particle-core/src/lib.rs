//! The particle engines behind the Effloresce Kits
//! (`plugins/kit/docs/implementation/particle-kit-plan.md`): Glint and
//! Pollen. Effloresce takes these over when it comes.
//!
//! Host-agnostic and allocation-free once built.

pub mod glint;
pub mod pollen;

pub use glint::Glint;
pub use pollen::Pollen;
