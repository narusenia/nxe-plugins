//! Presets (`v2-plan.md`, decision 17).
//!
//! **Factory presets are a table here**, applied through the same bracketed
//! edits a drag makes, so a host records loading one as an undoable change.
//! They set the macros and leave the voice shapes alone — the shape is the
//! doubler's character, and a preset that rewrote 32 shape values would be
//! impossible to read. **Their values are a first draft; settling them needs an
//! ear** (backlog).
//!
//! **User presets go to truce's `PresetStore`**, in the per-OS user directory
//! every format shares, so a host's own browser sees them too. They hold every
//! parameter.

use super::bind::{self, Ctx};
use crate::params::DoublerParams;
use truce_core::presets::{PresetRef, PresetStore};
use truce_core::state::hash_plugin_id;
use truce_params::Params;
use truce_utils::preset::PresetMeta;

/// One factory preset: a name and the macro values it sets, in their own units.
/// `voices` and `source` are indices into their choices.
pub struct Factory {
    pub name: &'static str,
    pub voices: u32,
    pub source: u32,
    pub detune: f64,
    pub delay: f64,
    pub spread: f64,
    pub humanize: f64,
    pub mix: f64,
}

pub const FACTORY: [Factory; 5] = [
    Factory {
        name: "Default",
        voices: 1,
        source: 0,
        detune: 12.0,
        delay: 22.0,
        spread: 0.7,
        humanize: 0.35,
        mix: 0.4,
    },
    Factory {
        name: "Tight",
        voices: 0,
        source: 0,
        detune: 6.0,
        delay: 12.0,
        spread: 0.45,
        humanize: 0.2,
        mix: 0.3,
    },
    Factory {
        name: "Wide",
        voices: 1,
        source: 0,
        detune: 14.0,
        delay: 28.0,
        spread: 1.0,
        humanize: 0.35,
        mix: 0.45,
    },
    Factory {
        name: "Choir",
        voices: 2,
        source: 0,
        detune: 18.0,
        delay: 34.0,
        spread: 0.85,
        humanize: 0.6,
        mix: 0.5,
    },
    Factory {
        name: "Stereo Keep",
        voices: 1,
        source: 1,
        detune: 10.0,
        delay: 20.0,
        spread: 0.6,
        humanize: 0.3,
        mix: 0.35,
    },
];

/// The values a factory preset writes, as `(id, plain)`.
pub fn values_of(params: &DoublerParams, preset: &Factory) -> [(u32, f64); 7] {
    [
        (params.voices.id(), f64::from(preset.voices)),
        (params.source.id(), f64::from(preset.source)),
        (params.detune.id(), preset.detune),
        (params.delay.id(), preset.delay),
        (params.spread.id(), preset.spread),
        (params.humanize.id(), preset.humanize),
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

/// The user library. `com.nxe.doubler2` is the id the state hash comes from,
/// so a preset saved by one format loads in another.
pub fn store() -> PresetStore {
    PresetStore::new("NXE", "Doubler 2", hash_plugin_id("com.nxe.doubler2"), None)
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
        let params = DoublerParams::default();
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
        let params = DoublerParams::default();
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
