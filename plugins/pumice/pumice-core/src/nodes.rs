//! Where the reduction is allowed to go, as a curve the user draws.
//!
//! **A node is a weight, never a filter** (`REQ-PUM-004`). It does not move a
//! gain of its own; it says how much of whatever the detector decides may be
//! applied at that frequency. `DEPTH` is still the amount, which is what keeps
//! `DEPTH` = 0 exactly nothing however the nodes are set.
//!
//! ## Bipolar, and that is the point
//!
//! A positive node deepens, a negative one **protects**. "Do not touch the
//! presence around 3 kHz" is a real thing to want on a vocal and a one-sided
//! control cannot say it. The sum is clamped to `0..=2`, so a node can double
//! the reduction or cancel it and no further.
//!
//! ## Six, because a host's parameters are static
//!
//! A DAW stores automation against parameter IDs, so a seventh node cannot
//! appear at run time — the count is fixed at build time and soothe's own is
//! too. Six is `6 × 4 = 24` parameters, the same order as Sparkleur's per-band
//! set.
//!
//! ## The default is none
//!
//! Pumice has no presets (`REQ-PUM-024`), so it has to work on the way in with
//! `DEPTH` alone. Nodes are for making exceptions to that, which means the
//! curve has to be exactly flat when none are on.

use crate::gain::range_into;

/// How many nodes a host will ever see. **Fixed at build time.**
pub const NODES: usize = 6;

/// One node, as the wrapper resolves it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub enabled: bool,
    pub freq_hz: f32,
    /// Full width at half maximum, in octaves.
    pub width_octaves: f32,
    /// `-1..=1`. Negative protects.
    pub depth: f32,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            enabled: false,
            freq_hz: 1_000.0,
            width_octaves: 0.5,
            depth: 0.5,
        }
    }
}

/// The band the plugin is allowed to work in at all.
///
/// **Folded in here rather than kept as its own pair of controls**
/// (`REQ-PUM-009`): the edges and the nodes are the same kind of statement
/// about frequency, and two places writing one curve is the problem
/// `REQ-GLU-009` describes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub low_hz: f32,
    pub high_hz: f32,
}

impl Default for Range {
    fn default() -> Self {
        Self {
            low_hz: 100.0,
            high_hz: 18_000.0,
        }
    }
}

/// The weight curve: the operating range, times one plus the nodes.
///
/// **Built when something moves, never per frame.** Six nodes against up to
/// 8193 bins is the one piece of arithmetic here that would be wasteful at hop
/// rate, and nothing about it depends on the signal.
pub fn weight_into(
    bins: usize,
    bin_hz: f32,
    nodes: &[Node; NODES],
    range: Range,
    edge_octaves: f32,
    out: &mut [f32],
) {
    range_into(bins, bin_hz, range.low_hz, range.high_hz, edge_octaves, out);

    let active = nodes.iter().filter(|node| node.enabled).count();
    if active == 0 {
        // Exactly flat inside the range, which is what `REQ-PUM-004` promises
        // and what lets `DEPTH` alone be the product on the way in.
        return;
    }

    for (bin, value) in out.iter_mut().enumerate().take(bins) {
        if *value == 0.0 {
            continue;
        }
        let hz = bin as f32 * bin_hz;
        if hz <= 0.0 {
            continue;
        }

        let mut sum = 1.0_f32;
        for node in nodes.iter().filter(|node| node.enabled) {
            sum += node.depth * bump(hz, node);
        }
        *value *= sum.clamp(0.0, 2.0);
    }
}

/// A gaussian on the log-frequency axis, one at the centre and a half at
/// `width_octaves / 2` away — so `width_octaves` is the full width at half
/// maximum, which is the number a person can read off a picture.
fn bump(hz: f32, node: &Node) -> f32 {
    if node.freq_hz <= 0.0 || node.width_octaves <= 0.0 {
        return 0.0;
    }
    let octaves = (hz / node.freq_hz).log2();
    let normalised = 2.0 * octaves / node.width_octaves;
    // `exp(−ln2·x²)` is `2^(−x²)`, and `exp2` is the one the hardware has.
    (-(normalised * normalised)).exp2()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINS: usize = 1025;
    const BIN_HZ: f32 = 23.437_5;

    fn weights(nodes: &[Node; NODES]) -> Vec<f32> {
        let mut out = vec![0.0; BINS];
        weight_into(BINS, BIN_HZ, nodes, Range::default(), 0.5, &mut out);
        out
    }

    fn at(curve: &[f32], hz: f32) -> f32 {
        curve[(hz / BIN_HZ).round() as usize]
    }

    fn one(node: Node) -> [Node; NODES] {
        let mut nodes = [Node::default(); NODES];
        nodes[0] = node;
        nodes
    }

    /// `REQ-PUM-004`: nothing on means exactly flat inside the range.
    #[test]
    fn no_nodes_is_exactly_flat() {
        let curve = weights(&[Node::default(); NODES]);
        for hz in [200.0, 500.0, 1_000.0, 4_000.0, 12_000.0] {
            assert!(
                (at(&curve, hz) - 1.0).abs() < 1e-6,
                "{hz} Hz reads {}",
                at(&curve, hz)
            );
        }
    }

    #[test]
    fn a_disabled_node_changes_nothing() {
        let flat = weights(&[Node::default(); NODES]);
        let off = weights(&one(Node {
            enabled: false,
            freq_hz: 2_500.0,
            width_octaves: 1.0,
            depth: 1.0,
        }));
        assert_eq!(flat, off);
    }

    #[test]
    fn a_zero_depth_node_changes_nothing() {
        let flat = weights(&[Node::default(); NODES]);
        let zero = weights(&one(Node {
            enabled: true,
            freq_hz: 2_500.0,
            width_octaves: 1.0,
            depth: 0.0,
        }));
        for (bin, value) in zero.iter().enumerate() {
            assert!((value - flat[bin]).abs() < 1e-6, "bin {bin}");
        }
    }

    /// `REQ-PUM-004`: negative protects.
    #[test]
    fn a_negative_node_protects_and_a_positive_one_deepens() {
        let deepen = weights(&one(Node {
            enabled: true,
            freq_hz: 2_500.0,
            width_octaves: 1.0,
            depth: 1.0,
        }));
        let protect = weights(&one(Node {
            enabled: true,
            freq_hz: 2_500.0,
            width_octaves: 1.0,
            depth: -1.0,
        }));

        assert!((at(&deepen, 2_500.0) - 2.0).abs() < 0.01);
        assert!(at(&protect, 2_500.0) < 0.01);
        // And neither reaches far from where it was put.
        assert!((at(&deepen, 300.0) - 1.0).abs() < 0.05);
        assert!((at(&protect, 300.0) - 1.0).abs() < 0.05);
    }

    /// `width` is the full width at half maximum, so a person reading it off
    /// the picture gets the number they see.
    #[test]
    fn width_is_the_full_width_at_half_maximum() {
        for width in [0.25_f32, 0.5, 1.0, 2.0] {
            let curve = weights(&one(Node {
                enabled: true,
                freq_hz: 2_000.0,
                width_octaves: width,
                depth: 1.0,
            }));
            let edge = 2_000.0 * (width * 0.5).exp2();
            // One plus half of the node's depth.
            assert!(
                (at(&curve, edge) - 1.5).abs() < 0.05,
                "width {width}: half-maximum at {edge} Hz reads {}",
                at(&curve, edge)
            );
        }
    }

    /// `REQ-PUM-004`: moving a node must not step.
    #[test]
    fn sweeping_a_node_does_not_step() {
        let mut previous: Option<Vec<f32>> = None;
        let mut hz = 300.0_f32;
        while hz < 8_000.0 {
            let curve = weights(&one(Node {
                enabled: true,
                freq_hz: hz,
                width_octaves: 0.5,
                depth: 1.0,
            }));
            if let Some(last) = &previous {
                let worst = curve
                    .iter()
                    .zip(last)
                    .fold(0.0_f32, |a, (x, y)| a.max((x - y).abs()));
                assert!(worst < 0.1, "{hz} Hz moved a weight by {worst}");
            }
            previous = Some(curve);
            hz *= 1.01;
        }
    }

    /// Two nodes on the same spot add, and the sum is capped rather than
    /// allowed to invert.
    #[test]
    fn overlapping_nodes_add_and_clamp() {
        let mut nodes = [Node::default(); NODES];
        for node in nodes.iter_mut().take(3) {
            *node = Node {
                enabled: true,
                freq_hz: 2_500.0,
                width_octaves: 1.0,
                depth: 1.0,
            };
        }
        let curve = weights(&nodes);
        assert!((at(&curve, 2_500.0) - 2.0).abs() < 1e-6, "not clamped");

        for node in nodes.iter_mut().take(3) {
            node.depth = -1.0;
        }
        let curve = weights(&nodes);
        assert!(at(&curve, 2_500.0) >= 0.0, "went negative");
    }

    /// The range still wins: a node outside it does nothing.
    #[test]
    fn a_node_outside_the_range_does_nothing() {
        let curve = weights(&one(Node {
            enabled: true,
            freq_hz: 50.0,
            width_octaves: 0.5,
            depth: 1.0,
        }));
        assert!(at(&curve, 50.0) < 1e-6, "{}", at(&curve, 50.0));
    }
}
