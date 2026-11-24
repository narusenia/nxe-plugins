//! The standalone build: `cargo truce run -p doubler`. Not shipped.

fn main() {
    truce_standalone::run::<doubler::Plugin>();
}
