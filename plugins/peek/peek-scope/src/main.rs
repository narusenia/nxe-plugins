//! The standalone build: `cargo truce run -p peek-scope`. Not shipped.

fn main() {
    truce_standalone::run::<peek_scope::Plugin>();
}
