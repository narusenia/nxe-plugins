//! The standalone build: `cargo truce run -p air`. Not shipped.

fn main() {
    truce_standalone::run::<air::Plugin>();
}
