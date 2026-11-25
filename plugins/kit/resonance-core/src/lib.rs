//! The resonance engines behind the Colour Kits
//! (`plugins/kit/docs/implementation/resonance-kit-plan.md`): Dew first, Halo
//! next, and the Colour flagships built from them: Meniscus (Dew's surface)
//! and Prisma (Halo's resonators).
//!
//! Host-agnostic and allocation-free once built.

pub mod dew;
pub mod halo;
pub mod meniscus;
pub mod prisma;

pub use dew::Dew;
pub use halo::Halo;
pub use meniscus::Meniscus;
pub use prisma::Prisma;
