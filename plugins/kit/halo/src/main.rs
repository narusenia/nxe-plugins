//! The standalone build: `cargo truce run -p halo`. Not shipped.

fn main() {
    truce_standalone::run::<halo::Plugin>();
}
