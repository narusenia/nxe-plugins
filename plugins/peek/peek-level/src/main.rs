//! The standalone build: `cargo truce run -p peek-level`. Not shipped.

fn main() {
    truce_standalone::run::<peek_level::Plugin>();
}
