//! The measurements behind the Peek meters.
//!
//! **One implementation per measurement**, read by the single-measurement
//! plugin and by `Peek` alike, so the two cannot disagree
//! (`plugins/peek/docs/implementation/peek-plan.md`, decision 2).
//!
//! Nothing here touches the signal: a meter reads its input and the plugin
//! passes that input through untouched.

pub mod level;
pub mod true_peak;

pub use level::ChannelLevel;
pub use true_peak::TruePeak;
