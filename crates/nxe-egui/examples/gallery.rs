//! Every widget in `nxe-egui`, in a plain desktop window, at the plugin
//! window's width. `cargo run -p nxe-egui --example gallery`.
//!
//! The top half is the H4 layout put together from the parts; the bottom is
//! the parts on their own. The segment at the top switches the palette, so a
//! change can be checked against all six accents without a DAW.

use eframe::egui;
use egui::{Rect, Ui, pos2, vec2};
use nxe_egui::curve::curve;
use nxe_egui::knob::{Control, block_height, knob};
use nxe_egui::preset::{PresetAction, preset_bar, step};
use nxe_egui::readout::{figure, readout};
use nxe_egui::sidebar::{Subject, sidebar};
use nxe_egui::theme::{Palette, SIDEBAR_WIDTH, SPACE_2, SPACE_3, SPACE_4, WINDOW_WIDTH};
use nxe_egui::{bar, font, meter, segment, status, surface, window};

const FIGURE_HEIGHT: f32 = 200.0;
const ROW_HEIGHT: f32 = 160.0;
const KNOB: f32 = 44.0;

struct Gallery {
    palette: usize,
    macros: [f32; 4],
    bars: [f32; 4],
    mode: usize,
    presets: Vec<String>,
    current: Option<usize>,
    modified: bool,
    loose: f32,
    started: std::time::Instant,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            palette: 0,
            macros: [0.3, 0.5, 0.7, 0.4],
            bars: [0.5, 0.25, 0.75, 0.6],
            mode: 0,
            presets: ["Default", "Wide", "Tight", "Chorus"]
                .map(String::from)
                .to_vec(),
            current: Some(0),
            modified: false,
            loose: 0.5,
            started: std::time::Instant::now(),
        }
    }
}

/// What every caller of a value control does with what it reports.
fn apply(control: &Control, value: &mut f32, default: f32, changed: &mut bool) {
    if let Some(next) = control.gestures.change {
        *value = next;
        *changed = true;
    }
    if control.gestures.reset {
        *value = default;
        *changed = true;
    }
    if let Some(typed) = control
        .typed
        .as_deref()
        .and_then(|typed| typed.trim().trim_end_matches('%').parse::<f32>().ok())
    {
        *value = (typed / 100.0).clamp(0.0, 1.0);
        *changed = true;
    }
}

impl eframe::App for Gallery {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if !window(ui) {
            return;
        }
        ui.ctx().request_repaint();
        let (name, palette) = Palette::ALL[self.palette];
        let palette = &palette;
        let time = self.started.elapsed().as_secs_f32();
        let mut changed = false;
        let width = WINDOW_WIDTH as f32;
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);

        ui.horizontal(|ui| {
            let names: Vec<&str> = Palette::ALL.iter().map(|(name, _)| *name).collect();
            if let Some(picked) = segment::segment(ui, palette, &names, self.palette, width - 320.0)
            {
                self.palette = picked;
            }
            ui.add_space(SPACE_4);
            match preset_bar(
                ui,
                palette,
                &self.presets,
                self.current,
                self.modified,
                320.0 - SPACE_4,
            ) {
                Some(PresetAction::Previous) => {
                    self.current = step(self.current, self.presets.len(), false)
                }
                Some(PresetAction::Next) => {
                    self.current = step(self.current, self.presets.len(), true)
                }
                Some(PresetAction::Load(index)) => self.current = Some(index),
                Some(PresetAction::Save(name)) => {
                    self.presets.push(name);
                    self.current = Some(self.presets.len() - 1);
                }
                None => return,
            }
            self.modified = false;
        });
        ui.add_space(SPACE_4);

        // The H4 layout.
        let height = FIGURE_HEIGHT + ROW_HEIGHT;
        ui.horizontal(|ui| {
            let subject = Subject {
                name: "Width",
                figure: &format!("{:.0}", self.macros[0] * 100.0),
                unit: "%",
                position: self.macros[0],
            };
            let macros = &mut self.macros;
            sidebar(ui, palette, name, &subject, height, |ui, ink| {
                ui.spacing_mut().item_spacing = vec2(0.0, SPACE_3);
                for row in macros.chunks_mut(2) {
                    ui.horizontal(|ui| {
                        for (index, value) in row.iter_mut().enumerate() {
                            let label = ["Detune", "Delay"][index];
                            let control = knob(
                                ui,
                                ink,
                                label,
                                *value,
                                &format!("{:.0}%", *value * 100.0),
                                KNOB,
                            );
                            apply(&control, value, 0.5, &mut changed);
                            ui.add_space(SPACE_4);
                        }
                    });
                }
            });
            ui.vertical(|ui| {
                surface::figure(
                    ui,
                    palette,
                    vec2(width - SIDEBAR_WIDTH, FIGURE_HEIGHT),
                    |painter, rect, ink| {
                        let points: Vec<(f32, f32)> = (0..=24)
                            .map(|i| {
                                let x = i as f32 / 24.0;
                                let y = 0.45
                                    + 0.3 * (x * 9.0 + time * 1.3).sin() * (1.0 - x)
                                    + 0.1 * (x * 23.0 - time).sin();
                                (x, y)
                            })
                            .collect();
                        let inner = rect.shrink(SPACE_4);
                        curve(
                            painter,
                            inner,
                            &points,
                            ink.ink,
                            Some(ink.ink.at(0.18)),
                            true,
                        );
                        font::tracked(
                            painter,
                            rect.left_top() + vec2(SPACE_3, SPACE_3),
                            egui::Align2::LEFT_TOP,
                            "Spectrum",
                            9.0,
                            ink.muted.color(),
                        );
                    },
                );
                ui.add_space(SPACE_3);
                ui.horizontal(|ui| {
                    ui.add_space(SPACE_3);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = vec2(0.0, SPACE_2);
                        for (index, value) in self.bars.iter_mut().enumerate() {
                            let bipolar = index % 2 == 0;
                            let gestures =
                                bar::bar(ui, palette, *value, bipolar, vec2(220.0, 16.0), 6.0);
                            if let Some(next) = gestures.change {
                                *value = next;
                                changed = true;
                            }
                            if gestures.reset {
                                *value = if bipolar { 0.5 } else { 0.0 };
                                changed = true;
                            }
                        }
                    });
                    ui.add_space(SPACE_4);
                    let (meters, _) =
                        ui.allocate_exact_size(vec2(40.0, 100.0), egui::Sense::hover());
                    for channel in 0..2 {
                        let level = 0.55 + 0.35 * (time * (2.0 + channel as f32)).sin().abs();
                        let rect = Rect::from_min_size(
                            pos2(meters.left() + channel as f32 * 14.0, meters.top()),
                            vec2(8.0, meters.height()),
                        );
                        meter::meter(ui.painter(), palette, rect, level, 0.95, true);
                    }
                    ui.vertical(|ui| {
                        readout(
                            ui,
                            palette,
                            "Output",
                            &figure(Some(-3.2 + time.sin()), 1),
                            "dB",
                            140.0,
                        );
                        ui.add_space(SPACE_3);
                        readout(ui, palette, "Silence", &figure(None, 1), "dB", 140.0);
                    });
                });
            });
        });
        ui.add_space(SPACE_4);

        // The parts on their own.
        ui.horizontal(|ui| {
            ui.add_space(SPACE_3);
            for size in [32.0, 44.0, 56.0] {
                let control = knob(
                    ui,
                    palette,
                    "Loose",
                    self.loose,
                    &format!("{:.0}%", self.loose * 100.0),
                    size,
                );
                apply(&control, &mut self.loose, 0.5, &mut changed);
                ui.add_space(SPACE_4);
            }
            ui.vertical(|ui| {
                if let Some(mode) =
                    segment::segment(ui, palette, &["Stereo", "Mono", "Mid"], self.mode, 240.0)
                {
                    self.mode = mode;
                    changed = true;
                }
                ui.add_space(SPACE_3);
                let (rect, _) =
                    ui.allocate_exact_size(vec2(240.0, block_height(0.0)), egui::Sense::hover());
                font::tracked(
                    ui.painter(),
                    rect.left_top(),
                    egui::Align2::LEFT_TOP,
                    "Cmd-click a knob, or click its figure, to type",
                    9.0,
                    palette.subtle.color(),
                );
            });
        });
        if changed && self.current.is_some() {
            self.modified = true;
        }

        let rest = ui.available_height() - status::HEIGHT;
        ui.add_space(rest.max(0.0));
        let hint = ui
            .ctx()
            .is_pointer_over_egui()
            .then_some("Drag up or down. Shift for fine, double-click to reset.");
        status::status(ui, palette, width, hint);
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH as f32, 620.0])
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "nxe-egui gallery",
        options,
        Box::new(|_| Ok(Box::<Gallery>::default())),
    )
}
