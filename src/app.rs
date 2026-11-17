//! UI: "send | receive" across the top, two big buttons, one text box.

use std::time::Instant;

use eframe::egui::{
	self, scroll_area::ScrollBarVisibility, text::LayoutJob, Align, Color32, FontId, Key, Layout,
	Modifiers, RichText, TextFormat,
};

use morse_trainer::compare::{self, Comparison, DiffOp};
use morse_trainer::generator::{self, GenKind, Rng};
use morse_trainer::morse::{self, Element, Spacing, Symbol, TapDecoder, Timing};

use crate::audio::Audio;

/// How long a button stays lit after its shortcut is pressed.
const FLASH_MS: f64 = 120.0;

const CONFIG_SIZE: [f32; 2] = [440.0, 220.0];

const RED: Color32 = Color32::from_rgb(220, 70, 60);

struct Config {
	wpm: f32,
	spacing: Spacing,
	effective_wpm: f32,
	pitch: f32,
	volume: f32,
}

impl Config {
	fn timing(&self) -> Timing {
		Timing::with_spacing(self.wpm, self.effective_wpm, self.spacing)
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
	Send,
	Receive,
}

pub struct MorseApp {
	audio: Option<Audio>,
	cfg: Config,
	show_config: bool,
	/// Screen position for the config window, fixed when it's opened.
	config_pos: egui::Pos2,
	tab: Tab,
	clock: Instant,

	taps: TapDecoder,
	flash: [f64; 2],

	rng: Rng,
	message: String,
	heard: String,
	result: Option<Comparison>,
}

impl MorseApp {
	pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
		let mut rng = Rng::from_time();
		let message = generator::generate(GenKind::Words, 4, &mut rng);
		Self {
			audio: Audio::new().ok(),
			cfg: Config {
				wpm: 18.0,
				spacing: Spacing::Standard,
				effective_wpm: 10.0,
				pitch: 600.0,
				volume: 0.5,
			},
			show_config: false,
			config_pos: egui::Pos2::ZERO,
			tab: Tab::Send,
			clock: Instant::now(),
			taps: TapDecoder::new(Timing::new(18.0)),
			flash: [f64::NEG_INFINITY; 2],
			rng,
			message,
			heard: String::new(),
			result: None,
		}
	}

	fn now(&self) -> f64 {
		self.clock.elapsed().as_secs_f64() * 1000.0
	}

	fn tap(&mut self, sym: Symbol) {
		let now = self.now();
		self.taps.tap(sym, now);
		let t = self.cfg.timing();
		let (i, ms) = match sym {
			Symbol::Dit => (0, t.dit_ms()),
			Symbol::Dah => (1, t.dah_ms()),
		};
		self.flash[i] = now;
		if let Some(a) = &self.audio {
			a.tap(
				Element {
					on: true,
					ms,
					char_index: None,
				},
				t.intra_gap_ms(),
			);
		}
	}

	fn play(&mut self) {
		if self.result.is_some() {
			self.message = generator::generate(GenKind::Words, 4, &mut self.rng);
			self.heard.clear();
			self.result = None;
		}
		if let Some(a) = &self.audio {
			a.play(&morse::to_elements(&self.message, self.cfg.timing()));
		}
	}

	fn check(&mut self) {
		self.result = Some(compare::compare(&self.message, &self.heard));
	}

	/// Config in its own OS window (non-modal).
	fn config_window(&mut self, ctx: &egui::Context) {
		if !self.show_config {
			return;
		}
		let cfg = &mut self.cfg;
		let mut open = true;
		ctx.show_viewport_immediate(
			egui::ViewportId::from_hash_of("config"),
			egui::ViewportBuilder::default()
				.with_title("config")
				.with_inner_size(CONFIG_SIZE)
				.with_position(self.config_pos)
				.with_resizable(false),
			|ctx, class| {
				if class == egui::ViewportClass::Embedded {
					// Backend without multi-window support: fall back to an in-app window.
					egui::Window::new("config")
						.open(&mut open)
						.collapsible(false)
						.resizable(false)
						.show(ctx, |ui| config_ui(ui, cfg));
				} else {
					egui::CentralPanel::default().show(ctx, |ui| config_ui(ui, cfg));
					if ctx.input(|i| i.viewport().close_requested()) {
						open = false;
					}
				}
			},
		);
		self.show_config = open;
	}

	fn header(&mut self, ui: &mut egui::Ui) {
		let current = self.tab;
		let mut picked = current;
		let rect = ui
			.scope(|ui| {
				ui.columns(2, |cols| {
					for (col, (tab, name)) in cols
						.iter_mut()
						.zip([(Tab::Send, "send"), (Tab::Receive, "receive")])
					{
						col.vertical_centered(|ui| {
							let mut text = RichText::new(name).size(34.0);
							if tab != current {
								text = text.weak();
							}
							if ui.add(egui::Button::new(text).frame(false)).clicked() {
								picked = tab;
							}
						});
					}
				});
			})
			.response
			.rect;
		let stroke = ui.visuals().widgets.noninteractive.fg_stroke;
		ui.painter().vline(rect.center().x, rect.y_range(), stroke);
		ui.painter()
			.hline(rect.x_range(), rect.bottom() + 4.0, stroke);
		ui.add_space(8.0);

		if picked != current {
			if let Some(a) = &self.audio {
				a.stop();
			}
			self.tab = picked;
		}
	}

	/// Two labelled buttons centred side by side. Returns which one was clicked.
	fn two_buttons(
		ui: &mut egui::Ui,
		labels: [&str; 2],
		glyphs: [&str; 2],
		lit: [bool; 2],
	) -> [bool; 2] {
		let size = egui::vec2(170.0, 100.0);
		let gap = 90.0;
		let mut clicked = [false; 2];
		ui.add_space(24.0);
		ui.horizontal(|ui| {
			ui.add_space(((ui.available_width() - 2.0 * size.x - gap) / 2.0).max(0.0));
			for i in 0..2 {
				ui.allocate_ui_with_layout(
					egui::vec2(size.x, size.y + 30.0),
					Layout::top_down(Align::Center),
					|ui| {
						ui.label(RichText::new(labels[i]).size(20.0));
						let mut b =
							egui::Button::new(RichText::new(glyphs[i]).size(44.0)).min_size(size);
						if lit[i] {
							b = b.fill(ui.visuals().selection.bg_fill);
						}
						clicked[i] = ui.add(b).clicked();
					},
				);
				if i == 0 {
					ui.add_space(gap);
				}
			}
		});
		ui.add_space(24.0);
		clicked
	}

	fn send(&mut self, ui: &mut egui::Ui) {
		let now = self.now();
		let lit = [
			now - self.flash[0] < FLASH_MS,
			now - self.flash[1] < FLASH_MS,
		];
		let [dot, dash] = Self::two_buttons(ui, ["dot (J)", "dash (K)"], ["•", "—"], lit);
		if dot {
			self.tap(Symbol::Dit);
		}
		if dash {
			self.tap(Symbol::Dah);
		}
		self.taps.update(self.now());

		ui.horizontal(|ui| {
			ui.label(RichText::new("translated:").size(20.0));
			ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
				if ui.button("clear").clicked() {
					self.taps.clear();
				}
			});
		});
		egui::Frame::group(ui.style()).show(ui, |ui| {
			egui::ScrollArea::vertical()
				.auto_shrink([false, false])
				.max_height(110.0)
				.stick_to_bottom(true)
				.scroll_bar_visibility(ScrollBarVisibility::AlwaysVisible)
				.show(ui, |ui| {
					ui.add(
						egui::Label::new(RichText::new(self.taps.text()).monospace().size(24.0))
							.wrap(),
					);
				});
		});
	}

	fn receive(&mut self, ui: &mut egui::Ui) {
		let [play, check] = Self::two_buttons(
			ui,
			["play (Ctrl+P)", "check (Enter)"],
			["▶", "✔"],
			[false; 2],
		);
		if play {
			self.play();
		}
		if check {
			self.check();
		}

		ui.label(RichText::new("heard:").size(20.0));
		ui.add(
			egui::TextEdit::singleline(&mut self.heard)
				.font(FontId::monospace(24.0))
				.desired_width(f32::INFINITY),
		);

		if let Some(r) = &self.result {
			ui.add_space(12.0);
			let font = FontId::monospace(24.0);
			let ok = ui.visuals().text_color();
			let mut job = LayoutJob::default();
			for op in &r.ops {
				let (c, color) = match *op {
					DiffOp::Match(c) => (c, ok),
					DiffOp::Wrong { expected, .. } | DiffOp::Missing(expected) => (expected, RED),
					DiffOp::Extra(_) => continue,
				};
				job.append(&c.to_string(), 0.0, TextFormat::simple(font.clone(), color));
			}
			ui.label(job);
		}
	}
}

fn config_ui(ui: &mut egui::Ui, cfg: &mut Config) {
	egui::Grid::new("cfg")
		.num_columns(2)
		.spacing([16.0, 10.0])
		.show(ui, |ui| {
			ui.label("speed");
			ui.add(
				egui::Slider::new(&mut cfg.wpm, 5.0..=40.0)
					.step_by(1.0)
					.suffix(" WPM"),
			);
			ui.end_row();

			ui.label("pitch");
			ui.add(
				egui::Slider::new(&mut cfg.pitch, 400.0..=1000.0)
					.step_by(10.0)
					.suffix(" Hz"),
			);
			ui.end_row();

			ui.label("volume");
			ui.add(
				egui::Slider::new(&mut cfg.volume, 0.0..=1.0)
					.custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
			);
			ui.end_row();

			ui.label("timing");
			ui.horizontal(|ui| {
				ui.radio_value(&mut cfg.spacing, Spacing::Standard, "standard");
				ui.radio_value(&mut cfg.spacing, Spacing::Farnsworth, "Farnsworth");
				ui.radio_value(&mut cfg.spacing, Spacing::Wordsworth, "Wordsworth");
			});
			ui.end_row();

			ui.label("effective speed");
			let max = cfg.wpm;
			cfg.effective_wpm = cfg.effective_wpm.min(max);
			ui.add_enabled(
				cfg.spacing != Spacing::Standard,
				egui::Slider::new(&mut cfg.effective_wpm, 3.0..=max)
					.step_by(1.0)
					.suffix(" WPM"),
			);
			ui.end_row();
		});
}

impl eframe::App for MorseApp {
	fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
		// Shortcuts, consumed before any widget can see them.
		match self.tab {
			Tab::Send => {
				let (j, k) = ctx.input_mut(|i| {
					(
						i.consume_key(Modifiers::NONE, Key::J),
						i.consume_key(Modifiers::NONE, Key::K),
					)
				});
				if j {
					self.tap(Symbol::Dit);
				}
				if k {
					self.tap(Symbol::Dah);
				}
			}
			Tab::Receive => {
				let (p, enter) = ctx.input_mut(|i| {
					(
						i.consume_key(Modifiers::COMMAND, Key::P),
						i.consume_key(Modifiers::NONE, Key::Enter),
					)
				});
				if p {
					self.play();
				}
				if enter {
					self.check();
				}
			}
		}

		if let Some(a) = &self.audio {
			a.set_tone(self.cfg.pitch, self.cfg.volume);
		}
		let timing = self.cfg.timing();
		if self.taps.timing() != timing {
			self.taps.set_timing(timing);
		}

		egui::TopBottomPanel::bottom("footer")
			.show_separator_line(false)
			.show(ctx, |ui| {
				ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
					if ui.toggle_value(&mut self.show_config, "config").clicked()
						&& self.show_config
					{
						// Centre it on the main window.
						let main = ctx.input(|i| i.viewport().outer_rect);
						let center = main.map_or(egui::pos2(400.0, 300.0), |r| r.center());
						self.config_pos = center - egui::Vec2::from(CONFIG_SIZE) / 2.0;
					}
				});
			});

		self.config_window(ctx);

		egui::CentralPanel::default().show(ctx, |ui| {
			self.header(ui);
			match self.tab {
				Tab::Send => self.send(ui),
				Tab::Receive => self.receive(ui),
			}
		});

		// Keep running so letters appear after the pause and buttons un-light.
		ctx.request_repaint();
	}
}
