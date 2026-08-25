//! Every `nxe-ui` widget as a plain desktop app, so UI work can be iterated
//! without launching a DAW. A widget that is not in here cannot be reviewed
//! without one, so it will not be (`.agents/rules/vizia.md`).
//!
//! This opens a standalone **baseview** window, not a winit one: the two vizia
//! backends are mutually exclusive and the plugin needs baseview (see the
//! `vizia` entry in the workspace `Cargo.toml`). The upside is that the
//! gallery runs on the same backend the plugin does.
//!
//! Run it with `mise run gallery`.

use vizia::prelude::*;

fn main() {
    Application::new(|cx| {
        Label::new(cx, "nxe-ui gallery");
    })
    .title("nxe-ui gallery")
    .inner_size((900, 640))
    .run();
}
