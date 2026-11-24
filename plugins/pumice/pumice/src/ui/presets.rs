//! The factory presets, and the user's own through truce's preset store.
//!
//! **A factory preset writes the main controls and the range, never the
//! nodes.** Nodes mark where *this* voice rings; a preset that moved them
//! would undo the one part of the setup that cannot be general.
//!
//! Only `Default` is the plugin's own defaults; the others are starting points
//! nobody has listened to yet (`v2-plan.md`, V2-16).

use super::bind::{self, Ctx};
use crate::params::PumiceParams;
use truce_core::presets::{PresetRef, PresetStore};
use truce_core::state::hash_plugin_id;
use truce_params::Params;
use truce_utils::preset::PresetMeta;

/// One factory preset: a name and the values it sets, in their own units.
pub struct Factory {
    pub name: &'static str,
    pub depth: f64,
    pub sharpness: f64,
    pub speed: f64,
    pub threshold: f64,
    pub mix: f64,
    pub low: f64,
    pub high: f64,
}

macro_rules! factory {
    ($name:literal: $depth:literal $sharpness:literal $speed:literal $threshold:literal
     $mix:literal $low:literal $high:literal) => {
        Factory {
            name: $name,
            depth: $depth,
            sharpness: $sharpness,
            speed: $speed,
            threshold: $threshold,
            mix: $mix,
            low: $low,
            high: $high,
        }
    };
}

pub const FACTORY: [Factory; 6] = [
    factory!("Default":     0.50 0.50 0.50 4.5 1.00 100.0 18000.0),
    factory!("Gentle":      0.30 0.40 0.40 6.0 1.00 100.0 18000.0),
    factory!("Firm":        0.75 0.55 0.60 3.0 1.00 100.0 18000.0),
    factory!("Harsh Top":   0.65 0.65 0.70 3.5 1.00 2000.0 16000.0),
    factory!("Boxy Mids":   0.55 0.35 0.40 4.0 1.00 150.0 1500.0),
    factory!("Parallel":    0.80 0.50 0.50 3.0 0.50 100.0 18000.0),
];

pub fn values_of(params: &PumiceParams, preset: &Factory) -> [(u32, f64); 7] {
    [
        (params.depth.id(), preset.depth),
        (params.sharpness.id(), preset.sharpness),
        (params.speed.id(), preset.speed),
        (params.threshold.id(), preset.threshold),
        (params.mix.id(), preset.mix),
        (params.low.id(), preset.low),
        (params.high.id(), preset.high),
    ]
}

pub fn apply(ctx: &Ctx, values: &[(u32, f64)]) {
    for (id, plain) in values {
        bind::set_once(ctx, *id, bind::normalize(ctx, *id, *plain));
    }
}

/// Whether the current values are this factory preset's.
pub fn matches(ctx: &Ctx, preset: &Factory) -> bool {
    values_of(ctx.params(), preset)
        .iter()
        .all(|(id, plain)| (f64::from(bind::plain(ctx, *id)) - plain).abs() < 1e-3)
}

/// The user library. `com.nxe.pumice` is the id the state hash comes from,
/// so a preset saved by one format loads in another.
pub fn store() -> PresetStore {
    PresetStore::new("NXE", "NXE Pumice", hash_plugin_id("com.nxe.pumice"), None)
}

pub fn user_presets(store: &PresetStore) -> Vec<PresetRef> {
    store
        .enumerate()
        .into_iter()
        .filter(|preset| matches!(preset.scope, truce_core::presets::PresetScope::User))
        .collect()
}

/// Saves every parameter under `name`. Returns the error as text for the
/// window to show.
pub fn save(ctx: &Ctx, store: &PresetStore, name: &str) -> Result<(), String> {
    let (ids, values) = ctx.params().collect_values();
    let pairs: Vec<(u32, f64)> = ids.into_iter().zip(values).collect();
    let meta = PresetMeta {
        name: name.to_owned(),
        category: "User".to_owned(),
        ..PresetMeta::default()
    };
    store
        .save(meta, &pairs, &[])
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

pub fn load(ctx: &Ctx, store: &PresetStore, preset: &PresetRef) -> Result<(), String> {
    let state = store
        .load(&preset.uri)
        .map_err(|error| format!("{error:?}"))?;
    apply(ctx, &state.params);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A preset that sets a value outside its parameter's range would be
    /// clamped silently, and the name would no longer say what is heard.
    #[test]
    fn factory_presets_stay_in_range() {
        let params = PumiceParams::default();
        let infos = params.param_infos();
        for preset in &FACTORY {
            for (id, plain) in values_of(&params, preset) {
                let info = infos.iter().find(|info| info.id == id).unwrap();
                let back = info.range.denormalize(info.range.normalize(plain));
                assert!(
                    (back - plain).abs() < 1e-6,
                    "{} sets {} to {plain}, out of range",
                    preset.name,
                    info.name
                );
            }
        }
    }

    /// The first factory preset is the plugin's own defaults, so "Default"
    /// means what it says.
    #[test]
    fn the_default_preset_is_the_defaults() {
        let params = PumiceParams::default();
        let infos = params.param_infos();
        for (id, plain) in values_of(&params, &FACTORY[0]) {
            let info = infos.iter().find(|info| info.id == id).unwrap();
            assert!(
                (info.default_plain - plain).abs() < 1e-6,
                "{} defaults to {}, not {plain}",
                info.name,
                info.default_plain
            );
        }
    }
}
