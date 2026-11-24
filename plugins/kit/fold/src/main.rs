//! The standalone build: `cargo truce run -p fold`. Not shipped.

fn main() {
    truce_standalone::run::<fold::Plugin>();
}
