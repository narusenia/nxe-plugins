//! The resonance engines behind the Colour Kits
//! (`plugins/kit/docs/implementation/resonance-kit-plan.md`): Dew first, Halo
//! next. Meniscus and Prisma take these over when they come.
//!
//! Host-agnostic and allocation-free once built.

pub mod dew;
pub mod halo;

pub use dew::Dew;
pub use halo::Halo;
