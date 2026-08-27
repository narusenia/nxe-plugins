//! A periodic message to a window, and a way to stop it.
//!
//! A plugin's display has to re-read what the audio thread published even when
//! nothing else is happening: parameter changes wake the binding system on
//! their own, an idle window with audio running does not.
//!
//! ## Why a thread and not `cx.add_timer`
//!
//! **vizia's timers never fire here.** `process_timers` is called by
//! `vizia_winit` and by nothing else; the baseview backend — the one every
//! plugin and the gallery run on — does not call it
//! (`.agents/rules/vizia.md`). `cx.spawn` hands out a `ContextProxy`, and
//! baseview does install an event proxy for that.
//!
//! ## Why the thread cannot use `emit` as its exit condition
//!
//! **`ContextProxy::emit` never fails on baseview.** The proxy is
//! `vizia_baseview::proxy::BaseviewProxy`, whose `send` pushes onto a
//! `lazy_static` queue and returns `Ok(())` unconditionally — it holds no
//! reference to a window and cannot know whether one is still there. The
//! obvious loop,
//!
//! ```ignore
//! while proxy.emit(Poll).is_ok() {
//!     std::thread::sleep(interval);
//! }
//! ```
//!
//! therefore **never ends**. Every time a host opens a plugin's window a new
//! thread starts, and closing the window does not stop it: the threads
//! accumulate for the life of the host process, each pushing an event onto a
//! **process-global, mutex-guarded, unbounded** queue thirty times a second.
//!
//! Worse, that queue is only drained by `Application::on_frame_update` — so
//! while every editor is closed **nothing drains it at all** and it grows
//! without limit, and the next window to open pays for all of it in one frame.
//!
//! So the exit condition has to be something the caller owns. [`start`] returns
//! a [`Lifeline`]; **put it in the model**, which the context drops when the
//! window closes, and the thread stops within one interval.

use std::any::Any;
use std::sync::{Arc, Weak};
use std::time::Duration;

use vizia::prelude::*;

/// Keeps a heartbeat running. **Store it in the model** — when the window's
/// context is dropped, so is this, and the thread stops.
///
/// Dropping it earlier stops the heartbeat earlier, which is the only other
/// thing it is for.
pub struct Lifeline(#[allow(dead_code)] Arc<()>);

/// Whether the window that asked for the heartbeat is still there.
fn still_wanted(token: &Weak<()>) -> bool {
    token.upgrade().is_some()
}

/// Emits `message` to the window every `interval` until the returned
/// [`Lifeline`] is dropped.
pub fn start<M>(cx: &mut Context, interval: Duration, message: M) -> Lifeline
where
    M: Any + Send + Clone + 'static,
{
    let alive = Arc::new(());
    let token = Arc::downgrade(&alive);

    cx.spawn(move |proxy| {
        // **Checked before the emit, not after.** A window that closed during
        // the sleep must not receive one more event on its way out.
        while still_wanted(&token) {
            if proxy.emit(message.clone()).is_err() {
                return;
            }
            std::thread::sleep(interval);
        }
    });

    Lifeline(alive)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The lifeline is the exit condition**, and it is one the caller
    /// controls — unlike `emit`, which on baseview reports success whether or
    /// not there is a window left to receive anything.
    #[test]
    fn the_heartbeat_stops_when_its_lifeline_is_dropped() {
        let alive = Arc::new(());
        let token = Arc::downgrade(&alive);
        assert!(still_wanted(&token));

        drop(alive);
        assert!(!still_wanted(&token));
    }

    /// And a clone of the lifeline keeps it running: a model moved rather than
    /// copied must not stop the display.
    #[test]
    fn a_second_holder_keeps_it_running() {
        let alive = Arc::new(());
        let token = Arc::downgrade(&alive);
        let second = alive.clone();

        drop(alive);
        assert!(still_wanted(&token));
        drop(second);
        assert!(!still_wanted(&token));
    }
}
