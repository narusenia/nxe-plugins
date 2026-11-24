//! The measurements behind the Peek meters.
//!
//! **One implementation per measurement**, read by the single-measurement
//! plugin and by `Peek` alike, so the two cannot disagree
//! (`plugins/peek/docs/implementation/peek-plan.md`, decision 2).
//!
//! Nothing here touches the signal: a meter reads its input and the plugin
//! passes that input through untouched.

pub mod level;
pub mod loudness;
pub mod meters;
pub mod scope;
pub mod stereo;
pub mod true_peak;
pub mod vu;

pub use level::ChannelLevel;
pub use loudness::Loudness;
pub use scope::Scope;
pub use stereo::Stereo;
pub use true_peak::TruePeak;
pub use vu::Vu;
