//! The standalone build: `cargo truce run -p peek-vu`. Not shipped.

fn main() {
    truce_standalone::run::<peek_vu::Plugin>();
}
