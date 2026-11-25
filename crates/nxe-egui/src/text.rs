//! Reading a value back from the text a user typed into a value entry.
//!
//! **Plain functions on strings**, so `nxe-egui` still knows nothing about a
//! plugin framework: a plugin's parameter declares `parse = "..."` and its
//! method calls one of these. The text is what the parameter printed, edited —
//! "35%", "+20 %", "-3 dB", "12.0 ct", "50L", "Off" — so each reads the number
//! and ignores the unit after it.
//!
//! **Every parameter of a plugin needs one** once any does: a host or a
//! validator that finds one parameter reading its text back expects them all
//! to, and clap-validator's `param-conversions` fails on the first that
//! cannot (`v2-plan.md`, V2-9).

/// The leading number of `text`, whatever unit follows it.
pub fn number(text: &str) -> Option<f64> {
    let text = text.trim();
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
        .unwrap_or(text.len());
    text[..end].parse().ok()
}

/// A frequency: "1.2 kHz", "350 Hz" or a bare number, in Hz.
pub fn hz(text: &str) -> Option<f64> {
    let value = number(text)?;
    Some(if text.to_ascii_lowercase().contains('k') {
        value * 1_000.0
    } else {
        value
    })
}

/// The nearest note's name to `hz`, with its octave: 220 → "A3".
pub fn note(hz: f32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let midi = (69.0 + 12.0 * (hz / 440.0).log2()).round() as i32;
    format!(
        "{}{}",
        NAMES[midi.rem_euclid(12) as usize],
        midi.div_euclid(12) - 1
    )
}

/// A percentage, as a fraction: "35%" → 0.35.
pub fn percent(text: &str) -> Option<f64> {
    number(text).map(|value| value / 100.0)
}

/// "On" / "Off", as a switch prints.
pub fn switch(text: &str) -> Option<f64> {
    match text.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" => Some(1.0),
        "off" | "0" | "false" => Some(0.0),
        _ => None,
    }
}

/// The index of `text` among `names`, ignoring case.
pub fn choice(names: &[&str], text: &str) -> Option<f64> {
    let text = text.trim();
    names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(text))
        .map(|index| index as f64)
}

/// A pan as truce prints it — "C", "50L", "50R" — or as a signed percentage,
/// to `-1..=1`.
pub fn pan(text: &str) -> Option<f64> {
    let text = text.trim().to_ascii_uppercase();
    if text == "C" {
        return Some(0.0);
    }
    if let Some(left) = text.strip_suffix('L').or_else(|| text.strip_prefix('L')) {
        return percent(left).map(|value| -value.abs());
    }
    if let Some(right) = text.strip_suffix('R').or_else(|| text.strip_prefix('R')) {
        return percent(right).map(f64::abs);
    }
    percent(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_reads_what_it_is_given() {
        assert_eq!(number("-3 dB"), Some(-3.0));
        assert_eq!(number("+0.50 oct"), Some(0.5));
        assert_eq!(number("12.0 ct"), Some(12.0));
        assert_eq!(number("loud"), None);
        assert_eq!(percent("35%"), Some(0.35));
        assert_eq!(percent("+20 %"), Some(0.2));
        assert_eq!(switch("Off"), Some(0.0));
        assert_eq!(switch("on"), Some(1.0));
        assert_eq!(
            choice(&["Mono Sum", "True Stereo"], "true stereo"),
            Some(1.0)
        );
        assert_eq!(choice(&["2", "4", "8"], "16"), None);
        assert_eq!(pan("C"), Some(0.0));
        assert_eq!(pan("50L"), Some(-0.5));
        assert_eq!(pan("R25"), Some(0.25));
        assert_eq!(pan("-30"), Some(-0.3));
    }
}
