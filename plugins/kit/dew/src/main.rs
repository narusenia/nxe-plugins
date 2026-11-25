//! The standalone build: `cargo truce run -p dew`. Not shipped.

fn main() {
    truce_standalone::run::<dew::Plugin>();
}
