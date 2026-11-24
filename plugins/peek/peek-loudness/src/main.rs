//! The standalone build: `cargo truce run -p peek-loudness`. Not shipped.

fn main() {
    truce_standalone::run::<peek_loudness::Plugin>();
}
