// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;

fn main() -> eframe::Result {
	let options = eframe::NativeOptions {
		viewport: eframe::egui::ViewportBuilder::default()
			.with_title("Morse Trainer")
			.with_inner_size([720.0, 460.0])
			.with_min_inner_size([520.0, 320.0]),
		..Default::default()
	};
	eframe::run_native(
		"Morse Trainer",
		options,
		Box::new(|cc| Ok(Box::new(app::MorseApp::new(cc)))),
	)
}
