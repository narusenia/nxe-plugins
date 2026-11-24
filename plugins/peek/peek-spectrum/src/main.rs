//! The standalone build: `cargo truce run -p peek-spectrum`. Not shipped.

fn main() {
    truce_standalone::run::<peek_spectrum::Plugin>();
}
