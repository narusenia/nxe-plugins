//! The standalone build: `cargo truce run -p diorama`. Not shipped.

fn main() {
    truce_standalone::run::<diorama::Plugin>();
}
