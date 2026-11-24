//! The standalone build: `cargo truce run -p fever`. Not shipped.

fn main() {
    truce_standalone::run::<fever::Plugin>();
}
