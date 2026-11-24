//! Presets (`v2-plan.md`, decision 17).
//!
//! **Factory presets are a table here**, applied through the same bracketed
//! edits a drag makes, so a host records loading one as an undoable change.
//! They set the macros and leave the curve, the follow deviations and the
//! guard alone. **Their values are a first draft; settling them needs an ear**
//! (backlog). `Default` is the plugin's own defaults.
//!
//! **User presets go to truce's `PresetStore`**, in the per-OS user directory
//! every format shares, so a host's own browser sees them too. They hold every
//! parameter.

use super::bind::{self, Ctx};
use crate::params::AirParams;
use truce_core::presets::{PresetRef, PresetStore};
use truce_core::state::hash_plugin_id;
use truce_params::Params;
use truce_utils::preset::PresetMeta;

/// One factory preset: a name and the macro values it sets, in their own units.
pub struct Factory {
    pub name: &'static str,
    pub surface: f64,
    pub blend: f64,
    pub character: f64,
    pub focus: f64,
    pub width: f64,
    pub follow: f64,
    pub mix: f64,
}

macro_rules! factory {
    ($name:literal: $surface:literal $blend:literal $character:literal $focus:literal
     $width:literal $follow:literal $mix:literal) => {
        Factory {
            name: $name,
            surface: $surface,
            blend: $blend,
            character: $character,
            focus: $focus,
            width: $width,
            follow: $follow,
            mix: $mix,
        }
    };
}

/// Surface, Blend, Character, Focus, Width, Follow, Mix.
pub const FACTORY: [Factory; 6] = [
    factory!("Default":  0.50 0.50 0.72 -0.50 0.60 1.00 1.00),
    factory!("Silk":     0.40 0.30 0.50 -0.30 0.50 0.90 1.00),
    factory!("Breath":   0.55 0.75 0.60 -0.60 0.70 1.00 1.00),
    factory!("Shimmer":  0.60 0.20 0.85  0.00 0.40 0.80 1.00),
    factory!("Wide Air": 0.50 0.70 0.72 -0.40 1.00 1.00 1.00),
    factory!("Static":   0.45 0.50 0.72 -0.50 0.60 0.00 1.00),
];

/// The values a factory preset writes, as `(id, plain)`.
pub fn values_of(params: &AirParams, preset: &Factory) -> [(u32, f64); 7] {
    [
        (params.surface.id(), preset.surface),
        (params.blend.id(), preset.blend),
        (params.character.id(), preset.character),
        (params.focus.id(), preset.focus),
        (params.width.id(), preset.width),
        (params.follow.id(), preset.follow),
        (params.mix.id(), preset.mix),
    ]
}

/// Writes `(id, plain)` pairs as one bracketed edit each.
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

/// The user library. `com.nxe.air2` is the id the state hash comes from,
/// so a preset saved by one format loads in another.
pub fn store() -> PresetStore {
    PresetStore::new("NXE", "NXE Air 2", hash_plugin_id("com.nxe.air2"), None)
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
        let params = AirParams::default();
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
        let params = AirParams::default();
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
