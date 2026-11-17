//! Morse code core: character table, encoding/decoding, PARIS/Farnsworth timing,
//! and a timing-based decoder for straight-key input.
//!
//! Nothing in here depends on the UI or audio. Times are plain `f32`/`f64`
//! milliseconds so everything can be unit-tested deterministically.

/// Placeholder shown for a dot/dash sequence that is not in the table.
///
/// `'?'` is itself a valid Morse character (`..--..`), so a character that is
/// *not* in the table is used to keep the two distinguishable.
pub const UNKNOWN_CHAR: char = '#';

/// ITU Morse table: letters, digits and common punctuation.
const TABLE: &[(char, &str)] = &[
	('A', ".-"),
	('B', "-..."),
	('C', "-.-."),
	('D', "-.."),
	('E', "."),
	('F', "..-."),
	('G', "--."),
	('H', "...."),
	('I', ".."),
	('J', ".---"),
	('K', "-.-"),
	('L', ".-.."),
	('M', "--"),
	('N', "-."),
	('O', "---"),
	('P', ".--."),
	('Q', "--.-"),
	('R', ".-."),
	('S', "..."),
	('T', "-"),
	('U', "..-"),
	('V', "...-"),
	('W', ".--"),
	('X', "-..-"),
	('Y', "-.--"),
	('Z', "--.."),
	('0', "-----"),
	('1', ".----"),
	('2', "..---"),
	('3', "...--"),
	('4', "....-"),
	('5', "....."),
	('6', "-...."),
	('7', "--..."),
	('8', "---.."),
	('9', "----."),
	('.', ".-.-.-"),
	(',', "--..--"),
	('?', "..--.."),
	('\'', ".----."),
	('!', "-.-.--"),
	('/', "-..-."),
	('(', "-.--."),
	(')', "-.--.-"),
	('&', ".-..."),
	(':', "---..."),
	(';', "-.-.-."),
	('=', "-...-"),
	('+', ".-.-."),
	('-', "-....-"),
	('_', "..--.-"),
	('"', ".-..-."),
	('$', "...-..-"),
	('@', ".--.-."),
];

/// All characters the app can encode, in table order.
pub fn supported_chars() -> impl Iterator<Item = char> {
	TABLE.iter().map(|(c, _)| *c)
}

/// Dot/dash code for a single character (case-insensitive), if known.
pub fn encode_char(c: char) -> Option<&'static str> {
	let c = c.to_ascii_uppercase();
	TABLE.iter().find(|(ch, _)| *ch == c).map(|(_, code)| *code)
}

/// Character for a dot/dash sequence, if known.
pub fn decode_symbol(code: &str) -> Option<char> {
	TABLE.iter().find(|(_, c)| *c == code).map(|(ch, _)| *ch)
}

/// Encode text as dots and dashes: letters separated by a space, words by `" / "`.
/// Characters with no Morse equivalent are skipped.
pub fn encode(text: &str) -> String {
	text.split_whitespace()
		.map(|word| {
			word.chars()
				.filter_map(encode_char)
				.collect::<Vec<_>>()
				.join(" ")
		})
		.filter(|w| !w.is_empty())
		.collect::<Vec<_>>()
		.join(" / ")
}

/// Decode a typed dot/dash string.
///
/// * `.` is a dit; `-` or `_` is a dah.
/// * A single space separates letters.
/// * `/` (or `|`, or two or more spaces) separates words.
/// * Unknown sequences decode to [`UNKNOWN_CHAR`]; any other character acts
///   as a letter separator. Never panics.
pub fn decode(code: &str) -> String {
	let mut out = String::new();
	let mut symbol = String::new();
	let mut spaces = 0usize;
	let mut pending_word = false;

	fn flush(symbol: &mut String, out: &mut String) {
		if !symbol.is_empty() {
			out.push(decode_symbol(symbol).unwrap_or(UNKNOWN_CHAR));
			symbol.clear();
		}
	}

	for ch in code.chars() {
		match ch {
			'.' | '-' | '_' => {
				if symbol.is_empty() {
					if pending_word && !out.is_empty() {
						out.push(' ');
					}
					pending_word = false;
				}
				spaces = 0;
				symbol.push(if ch == '.' { '.' } else { '-' });
			}
			'/' | '|' => {
				flush(&mut symbol, &mut out);
				pending_word = true;
			}
			c if c.is_whitespace() => {
				flush(&mut symbol, &mut out);
				spaces += 1;
				if spaces >= 2 {
					pending_word = true;
				}
			}
			_ => flush(&mut symbol, &mut out),
		}
	}
	flush(&mut symbol, &mut out);
	out
}

// ---------------------------------------------------------------------------
// Timing
// ---------------------------------------------------------------------------

/// Speed settings. Element lengths follow the PARIS standard
/// (dit = 1 unit, dah = 3, intra-character gap = 1, letter gap = 3, word gap = 7,
/// with one unit = 1200 / WPM ms).
///
/// Characters are always sent at `wpm`. The spacing mode decides which gaps
/// get stretched to bring the overall rate down to `effective_wpm`:
///
/// * [`Spacing::Farnsworth`]: letter and word gaps (ARRL formula).
/// * [`Spacing::Wordsworth`]: only word gaps; letters within a word keep
///   standard spacing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spacing {
	Standard,
	Farnsworth,
	Wordsworth,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
	/// Character speed in words per minute.
	pub wpm: f32,
	/// Overall speed; equal to `wpm` for standard timing.
	pub effective_wpm: f32,
	pub spacing: Spacing,
}

impl Timing {
	/// Standard timing (no stretching).
	pub fn new(wpm: f32) -> Self {
		let wpm = wpm.max(1.0);
		Self {
			wpm,
			effective_wpm: wpm,
			spacing: Spacing::Standard,
		}
	}

	/// Stretched timing. The effective speed is clamped to the character speed.
	pub fn with_spacing(wpm: f32, effective_wpm: f32, spacing: Spacing) -> Self {
		let wpm = wpm.max(1.0);
		Self {
			wpm,
			effective_wpm: effective_wpm.clamp(1.0, wpm),
			spacing,
		}
	}

	/// Farnsworth timing: letter and word gaps stretched.
	pub fn with_farnsworth(wpm: f32, effective_wpm: f32) -> Self {
		Self::with_spacing(wpm, effective_wpm, Spacing::Farnsworth)
	}

	/// Wordsworth timing: only word gaps stretched.
	pub fn with_wordsworth(wpm: f32, effective_wpm: f32) -> Self {
		Self::with_spacing(wpm, effective_wpm, Spacing::Wordsworth)
	}

	/// One element unit at the character speed.
	pub fn unit_ms(&self) -> f32 {
		1200.0 / self.wpm
	}

	/// One unit of Farnsworth letter/word spacing (equal to
	/// [`unit_ms`](Self::unit_ms) in the other modes).
	pub fn spacing_unit_ms(&self) -> f32 {
		let c = self.wpm;
		let s = self.effective_wpm.min(c);
		if self.spacing != Spacing::Farnsworth || s >= c {
			return self.unit_ms();
		}
		// ARRL: total spacing time per PARIS word (19 spacing units), in seconds.
		let ta = (60.0 * c - 37.2 * s) / (s * c);
		ta * 1000.0 / 19.0
	}

	pub fn dit_ms(&self) -> f32 {
		self.unit_ms()
	}
	pub fn dah_ms(&self) -> f32 {
		3.0 * self.unit_ms()
	}
	pub fn intra_gap_ms(&self) -> f32 {
		self.unit_ms()
	}
	pub fn letter_gap_ms(&self) -> f32 {
		3.0 * self.spacing_unit_ms()
	}
	pub fn word_gap_ms(&self) -> f32 {
		match self.spacing {
			// A PARIS word is 43 units plus one word gap; size the gap so the
			// whole thing takes 60 / effective_wpm seconds.
			Spacing::Wordsworth => {
				let word_ms = 60_000.0 / self.effective_wpm.min(self.wpm);
				(word_ms - 43.0 * self.unit_ms()).max(7.0 * self.unit_ms())
			}
			_ => 7.0 * self.spacing_unit_ms(),
		}
	}
}

/// One tone or silence in a keyed sequence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Element {
	/// `true` for tone, `false` for silence.
	pub on: bool,
	pub ms: f32,
	/// Index (in `chars()`) of the source character this tone belongs to.
	/// `None` for gaps.
	pub char_index: Option<usize>,
}

/// Convert text to a tone/silence sequence. Unknown characters are skipped;
/// runs of whitespace become one word gap. No leading or trailing silence.
pub fn to_elements(text: &str, timing: Timing) -> Vec<Element> {
	let mut out: Vec<Element> = Vec::new();
	let mut word_break = false;
	let off = |ms| Element {
		on: false,
		ms,
		char_index: None,
	};

	for (i, ch) in text.chars().enumerate() {
		if ch.is_whitespace() {
			word_break = true;
			continue;
		}
		let Some(code) = encode_char(ch) else {
			continue;
		};
		if !out.is_empty() {
			out.push(off(if word_break {
				timing.word_gap_ms()
			} else {
				timing.letter_gap_ms()
			}));
		}
		word_break = false;
		for (k, sym) in code.chars().enumerate() {
			if k > 0 {
				out.push(off(timing.intra_gap_ms()));
			}
			out.push(Element {
				on: true,
				ms: if sym == '.' {
					timing.dit_ms()
				} else {
					timing.dah_ms()
				},
				char_index: Some(i),
			});
		}
	}
	out
}

/// Total duration of a sequence.
pub fn total_ms(elements: &[Element]) -> f32 {
	elements.iter().map(|e| e.ms).sum()
}

// ---------------------------------------------------------------------------
// Straight-key decoding
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
	Dit,
	Dah,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
	/// Between elements of one character.
	Intra,
	/// Between characters.
	Letter,
	/// Between words.
	Word,
}

/// How fast the adaptive estimates follow the operator (0..1).
const ADAPT_RATE: f32 = 0.3;

/// Turns key press/release timestamps into dits, dahs, letters and words.
///
/// Classification thresholds sit halfway between the ideal lengths:
/// press < 2 units → dit; gap < 2 units → intra-character; gap < 5 units →
/// letter gap; otherwise word gap (gap thresholds scale with Farnsworth spacing).
///
/// With `adaptive` on, running averages of the operator's actual dit and dah
/// lengths replace the nominal ones, and the gap thresholds are scaled by the
/// same factor.
#[derive(Clone, Debug)]
pub struct KeyDecoder {
	timing: Timing,
	adaptive: bool,
	dit_est: f32,
	dah_est: f32,
	down_at: Option<f64>,
	up_at: Option<f64>,
	letter_done: bool,
	word_done: bool,
	current: String,
	raw: String,
	text: String,
}

impl KeyDecoder {
	pub fn new(timing: Timing, adaptive: bool) -> Self {
		Self {
			timing,
			adaptive,
			dit_est: timing.dit_ms(),
			dah_est: timing.dah_ms(),
			down_at: None,
			up_at: None,
			letter_done: true,
			word_done: true,
			current: String::new(),
			raw: String::new(),
			text: String::new(),
		}
	}

	/// Change nominal speed; resets adaptive estimates but keeps decoded text.
	pub fn set_timing(&mut self, timing: Timing) {
		self.timing = timing;
		self.reset_estimates();
	}

	pub fn set_adaptive(&mut self, adaptive: bool) {
		self.adaptive = adaptive;
		self.reset_estimates();
	}

	fn reset_estimates(&mut self) {
		self.dit_est = self.timing.dit_ms();
		self.dah_est = self.timing.dah_ms();
	}

	pub fn timing(&self) -> Timing {
		self.timing
	}

	pub fn is_adaptive(&self) -> bool {
		self.adaptive
	}

	/// Current estimate of one element unit, in ms.
	pub fn unit_estimate_ms(&self) -> f32 {
		if self.adaptive {
			(self.dit_est + self.dah_est / 3.0) / 2.0
		} else {
			self.timing.unit_ms()
		}
	}

	/// Character speed implied by [`unit_estimate_ms`](Self::unit_estimate_ms).
	pub fn estimated_wpm(&self) -> f32 {
		1200.0 / self.unit_estimate_ms()
	}

	fn scale(&self) -> f32 {
		self.unit_estimate_ms() / self.timing.unit_ms()
	}

	/// Presses shorter than this are dits, longer are dahs.
	pub fn dit_dah_threshold_ms(&self) -> f32 {
		if self.adaptive {
			(self.dit_est + self.dah_est) / 2.0
		} else {
			2.0 * self.timing.unit_ms()
		}
	}

	/// Silences at least this long end a character.
	pub fn letter_gap_threshold_ms(&self) -> f32 {
		(self.timing.intra_gap_ms() + self.timing.letter_gap_ms()) / 2.0 * self.scale()
	}

	/// Silences at least this long end a word.
	pub fn word_gap_threshold_ms(&self) -> f32 {
		(self.timing.letter_gap_ms() + self.timing.word_gap_ms()) / 2.0 * self.scale()
	}

	pub fn classify_press(&self, ms: f32) -> Symbol {
		if ms < self.dit_dah_threshold_ms() {
			Symbol::Dit
		} else {
			Symbol::Dah
		}
	}

	pub fn classify_gap(&self, ms: f32) -> Gap {
		if ms < self.letter_gap_threshold_ms() {
			Gap::Intra
		} else if ms < self.word_gap_threshold_ms() {
			Gap::Letter
		} else {
			Gap::Word
		}
	}

	/// Key pressed at `t_ms`. Repeated calls while already down are ignored.
	pub fn key_down(&mut self, t_ms: f64) {
		if self.down_at.is_some() {
			return;
		}
		self.update(t_ms);
		self.down_at = Some(t_ms);
	}

	/// Key released at `t_ms`. Returns the symbol that was keyed, if any.
	pub fn key_up(&mut self, t_ms: f64) -> Option<Symbol> {
		let down = self.down_at.take()?;
		let ms = (t_ms - down).max(0.0) as f32;
		let sym = self.classify_press(ms);
		if self.adaptive {
			self.adapt(sym, ms);
		}
		let c = match sym {
			Symbol::Dit => '.',
			Symbol::Dah => '-',
		};
		self.current.push(c);
		self.raw.push(c);
		self.up_at = Some(t_ms);
		self.letter_done = false;
		self.word_done = false;
		Some(sym)
	}

	/// Call regularly (e.g. every frame) so letters and word spaces are
	/// emitted while the key is idle.
	pub fn update(&mut self, now_ms: f64) {
		if self.down_at.is_some() {
			return;
		}
		let Some(up) = self.up_at else { return };
		let gap = (now_ms - up).max(0.0) as f32;
		if !self.letter_done && gap >= self.letter_gap_threshold_ms() {
			self.text
				.push(decode_symbol(&self.current).unwrap_or(UNKNOWN_CHAR));
			self.current.clear();
			self.raw.push(' ');
			self.letter_done = true;
		}
		if !self.word_done && gap >= self.word_gap_threshold_ms() {
			self.text.push(' ');
			self.raw.push_str("/ ");
			self.word_done = true;
		}
	}

	fn adapt(&mut self, sym: Symbol, ms: f32) {
		// Ignore absurd values (contact bounce, key held while thinking).
		let ms = ms.clamp(10.0, 2000.0);
		// Update the observed class, then nudge the other one toward the ideal
		// 1:3 ratio. Without that coupling, an operator much faster than the
		// nominal speed (whose dahs all fall below the threshold) would never
		// pull the dah estimate down and every element would read as a dit.
		match sym {
			Symbol::Dit => {
				self.dit_est += ADAPT_RATE * (ms - self.dit_est);
				self.dah_est += ADAPT_RATE * (3.0 * self.dit_est - self.dah_est);
			}
			Symbol::Dah => {
				self.dah_est += ADAPT_RATE * (ms - self.dah_est);
				self.dit_est += ADAPT_RATE * (self.dah_est / 3.0 - self.dit_est);
			}
		}
		// Stay within ~4–60 WPM and keep a sane dah/dit ratio.
		self.dit_est = self.dit_est.clamp(20.0, 300.0);
		self.dah_est = self.dah_est.clamp(2.0 * self.dit_est, 5.0 * self.dit_est);
	}

	pub fn is_key_down(&self) -> bool {
		self.down_at.is_some()
	}

	/// Dots/dashes of the character being keyed right now.
	pub fn current_symbol(&self) -> &str {
		&self.current
	}

	/// Everything keyed so far, e.g. `".- -... / -.-. "`.
	pub fn raw(&self) -> &str {
		&self.raw
	}

	/// Decoded text so far.
	pub fn text(&self) -> &str {
		&self.text
	}

	/// Forget all keyed input (keeps timing and adaptive estimates).
	pub fn clear(&mut self) {
		self.down_at = None;
		self.up_at = None;
		self.letter_done = true;
		self.word_done = true;
		self.current.clear();
		self.raw.clear();
		self.text.clear();
	}
}

/// Decoder for separate dot and dash buttons.
///
/// Each tap is one element of the configured length. Taps that come faster
/// than the elements can sound are queued back to back (with the normal gap
/// between elements), just like the audio.
///
/// Letter and word breaks are timed from when the last element *starts*, and
/// are the same whether it was a dot or a dash: the next tap starts a new
/// letter if it comes at least a dah plus a letter gap later, and a new word
/// if it comes at least a dah plus half-way to a word gap later. So the pause
/// you need doesn't depend on which button you pressed last, and it scales
/// with speed and Farnsworth/Wordsworth spacing.
#[derive(Clone, Debug)]
pub struct TapDecoder {
	timing: Timing,
	/// When the last queued element stops sounding.
	busy_until: Option<f64>,
	/// When the last queued element starts sounding.
	last_start: Option<f64>,
	letter_done: bool,
	word_done: bool,
	current: String,
	text: String,
}

impl TapDecoder {
	pub fn new(timing: Timing) -> Self {
		Self {
			timing,
			busy_until: None,
			last_start: None,
			letter_done: true,
			word_done: true,
			current: String::new(),
			text: String::new(),
		}
	}

	pub fn timing(&self) -> Timing {
		self.timing
	}

	pub fn set_timing(&mut self, timing: Timing) {
		self.timing = timing;
	}

	/// Time after the last element starts at which the letter ends.
	pub fn letter_pause_ms(&self) -> f64 {
		(self.timing.dah_ms() + self.timing.letter_gap_ms()) as f64
	}

	/// Time after the last element starts at which the word ends.
	pub fn word_pause_ms(&self) -> f64 {
		let word_gap = (self.timing.letter_gap_ms() + self.timing.word_gap_ms()) / 2.0;
		(self.timing.dah_ms() + word_gap) as f64
	}

	pub fn tap(&mut self, sym: Symbol, now_ms: f64) {
		self.update(now_ms);
		let (c, ms) = match sym {
			Symbol::Dit => ('.', self.timing.dit_ms()),
			Symbol::Dah => ('-', self.timing.dah_ms()),
		};
		let start = match self.busy_until {
			Some(end) if end > now_ms => end + self.timing.intra_gap_ms() as f64,
			_ => now_ms,
		};
		self.busy_until = Some(start + ms as f64);
		self.last_start = Some(start);
		self.current.push(c);
		self.letter_done = false;
		self.word_done = false;
	}

	/// Call regularly so letters and spaces appear while no button is pressed.
	pub fn update(&mut self, now_ms: f64) {
		let Some(start) = self.last_start else { return };
		let pause = now_ms - start;
		if !self.letter_done && pause >= self.letter_pause_ms() {
			self.text
				.push(decode_symbol(&self.current).unwrap_or(UNKNOWN_CHAR));
			self.current.clear();
			self.letter_done = true;
		}
		if !self.word_done && pause >= self.word_pause_ms() {
			self.text.push(' ');
			self.word_done = true;
		}
	}

	/// Dots/dashes of the letter being entered.
	pub fn current_symbol(&self) -> &str {
		&self.current
	}

	pub fn text(&self) -> &str {
		&self.text
	}

	/// Forget everything entered so far.
	pub fn clear(&mut self) {
		self.busy_until = None;
		self.last_start = None;
		self.letter_done = true;
		self.word_done = true;
		self.current.clear();
		self.text.clear();
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// 20 WPM: dit 60, dah 180, element gap 60. A new letter starts 360 ms
	// after the last element starts (dah + letter gap), a new word 480 ms
	// after (dah + 300).

	#[test]
	fn tap_decoder_follows_speed() {
		let mut d = TapDecoder::new(Timing::new(20.0));
		for t in [0.0, 120.0, 240.0] {
			d.tap(Symbol::Dit, t);
		}
		d.update(590.0);
		assert_eq!(d.text(), "");
		assert_eq!(d.current_symbol(), "...");
		d.update(600.0);
		assert_eq!(d.text(), "S");
		// O in the same word: dahs start at 650, 900, 1150.
		for t in [650.0, 900.0, 1150.0] {
			d.tap(Symbol::Dah, t);
		}
		d.update(1500.0);
		assert_eq!(d.text(), "S");
		d.update(1510.0);
		assert_eq!(d.text(), "SO");
		d.update(1630.0);
		assert_eq!(d.text(), "SO ");
		d.update(9000.0);
		assert_eq!(d.text(), "SO ");
		d.clear();
		assert_eq!(d.text(), "");
		d.update(20_000.0);
		assert_eq!(d.text(), "");
	}

	#[test]
	fn tap_decoder_dot_and_dash_need_the_same_pause() {
		// Whatever was pressed first, the same gap between presses gives the
		// same split: under 360 ms one letter, at or over it two.
		for (gap, one_letter) in [(300.0, true), (350.0, true), (360.0, false), (400.0, false)] {
			let mut a = TapDecoder::new(Timing::new(20.0));
			a.tap(Symbol::Dit, 0.0);
			a.tap(Symbol::Dah, gap);
			a.update(10_000.0);
			let mut b = TapDecoder::new(Timing::new(20.0));
			b.tap(Symbol::Dah, 0.0);
			b.tap(Symbol::Dit, gap);
			b.update(10_000.0);
			let (want_a, want_b) = if one_letter {
				("A ", "N ")
			} else {
				("ET ", "TE ")
			};
			assert_eq!(a.text(), want_a, "dot then dash, {gap} ms");
			assert_eq!(b.text(), want_b, "dash then dot, {gap} ms");
		}
	}

	#[test]
	fn tap_decoder_queues_fast_taps() {
		let mut d = TapDecoder::new(Timing::new(20.0));
		// Three taps in 20 ms still sound as one letter: starts at 0, 120, 240.
		for t in [0.0, 10.0, 20.0] {
			d.tap(Symbol::Dit, t);
		}
		d.update(590.0);
		assert_eq!(d.text(), "");
		d.update(600.0);
		assert_eq!(d.text(), "S");
	}

	#[test]
	fn tap_decoder_speed_changes_breaks() {
		// The same taps that make one letter at 20 WPM are two letters at 40 WPM.
		let taps = [(Symbol::Dit, 0.0), (Symbol::Dah, 200.0)];
		for (wpm, want) in [(20.0, "A "), (40.0, "ET ")] {
			let mut d = TapDecoder::new(Timing::new(wpm));
			for (s, t) in taps {
				d.tap(s, t);
			}
			d.update(5000.0);
			assert_eq!(d.text(), want, "{wpm} WPM");
		}
	}

	#[test]
	fn tap_decoder_farnsworth_stretches_breaks() {
		// At 20/10 Farnsworth the letter gap is much longer than at plain 20.
		let mut d = TapDecoder::new(Timing::with_farnsworth(20.0, 10.0));
		d.tap(Symbol::Dit, 0.0);
		d.update(380.0); // past a plain 20 WPM letter break
		assert_eq!(d.text(), "");
		d.tap(Symbol::Dah, 400.0);
		d.update(5000.0);
		assert_eq!(d.text(), "A ");
	}

	#[test]
	fn tap_decoder_unknown() {
		let mut d = TapDecoder::new(Timing::new(20.0));
		for i in 0..8 {
			d.tap(Symbol::Dah, i as f64 * 100.0);
		}
		d.update(10_000.0);
		assert_eq!(d.text(), format!("{UNKNOWN_CHAR} "));
	}

	fn approx(a: f32, b: f32) -> bool {
		(a - b).abs() < 1e-3 * b.abs().max(1.0)
	}

	#[test]
	fn encode_basic() {
		assert_eq!(encode("SOS"), "... --- ...");
		assert_eq!(encode("sos"), "... --- ...");
		assert_eq!(encode("hi  there"), ".... .. / - .... . .-. .");
		assert_eq!(encode("  "), "");
	}

	#[test]
	fn encode_skips_unknown_chars() {
		assert_eq!(encode("A~B"), ".- -...");
		assert_eq!(encode("~ E"), ".");
	}

	#[test]
	fn decode_basic() {
		assert_eq!(decode("... --- ..."), "SOS");
		assert_eq!(decode(".... .. / - .... . .-. ."), "HI THERE");
		assert_eq!(decode(".... ..  - .... . .-. ."), "HI THERE");
		assert_eq!(decode(".... ../-"), "HI T");
		assert_eq!(decode("_._."), "C");
	}

	#[test]
	fn decode_edge_cases() {
		assert_eq!(decode(""), "");
		assert_eq!(decode("   /  / "), "");
		assert_eq!(decode("/ ... /"), "S");
		assert_eq!(decode("... / / ---"), "S O");
	}

	#[test]
	fn decode_unknown_gives_placeholder() {
		assert_eq!(decode("........"), UNKNOWN_CHAR.to_string());
		assert_eq!(decode(".- ........ -..."), format!("A{UNKNOWN_CHAR}B"));
		assert_eq!(decode_symbol("......."), None);
	}

	#[test]
	fn round_trip_every_character() {
		for c in supported_chars() {
			let code = encode_char(c).unwrap();
			assert_eq!(decode_symbol(code), Some(c), "char {c:?}");
			assert_eq!(decode(&encode(&c.to_string())), c.to_string());
		}
		let msg = "CQ CQ DE W1AW/3 K. QTH IS 52,3? RST=599 + (OK) @ \"HI\"";
		assert_eq!(decode(&encode(msg)), msg);
	}

	#[test]
	fn table_has_no_duplicates() {
		let all: Vec<_> = TABLE.iter().collect();
		for (i, (c1, k1)) in all.iter().enumerate() {
			for (c2, k2) in &all[i + 1..] {
				assert_ne!(c1, c2);
				assert_ne!(k1, k2, "{c1} and {c2} share a code");
			}
		}
	}

	#[test]
	fn paris_timing_is_50_units() {
		let t = Timing::new(20.0);
		assert!(approx(t.unit_ms(), 60.0));
		// "PARIS" is 43 units; plus the trailing 7-unit word gap = 50.
		let el = to_elements("PARIS", t);
		assert!(approx(total_ms(&el), 43.0 * 60.0));
		assert!(approx(total_ms(&el) + t.word_gap_ms(), 50.0 * 60.0));
		// Two words: 43 + 7 + 43.
		assert!(approx(
			total_ms(&to_elements("PARIS PARIS", t)),
			93.0 * 60.0
		));
	}

	#[test]
	fn element_lengths() {
		let t = Timing::new(10.0);
		assert!(approx(t.dit_ms(), 120.0));
		assert!(approx(t.dah_ms(), 360.0));
		assert!(approx(t.intra_gap_ms(), 120.0));
		assert!(approx(t.letter_gap_ms(), 360.0));
		assert!(approx(t.word_gap_ms(), 840.0));

		let el = to_elements("A E", t);
		let shape: Vec<(bool, f32)> = el.iter().map(|e| (e.on, e.ms)).collect();
		assert_eq!(
			shape,
			vec![
				(true, 120.0),
				(false, 120.0),
				(true, 360.0),
				(false, 840.0),
				(true, 120.0)
			]
		);
		assert_eq!(el[0].char_index, Some(0));
		assert_eq!(el[2].char_index, Some(0));
		assert_eq!(el[4].char_index, Some(2));
	}

	#[test]
	fn farnsworth_timing() {
		// Equal speeds are identical to standard timing.
		let std = Timing::new(18.0);
		let same = Timing::with_farnsworth(18.0, 18.0);
		assert!(approx(same.spacing_unit_ms(), std.unit_ms()));

		// 18 WPM characters at 5 WPM effective (ARRL example).
		let f = Timing::with_farnsworth(18.0, 5.0);
		assert!(approx(f.unit_ms(), 1200.0 / 18.0));
		let ta = (60.0 * 18.0 - 37.2 * 5.0) / (5.0 * 18.0) * 1000.0;
		assert!(approx(f.spacing_unit_ms(), ta / 19.0));
		// A full PARIS word (31 char units + 19 spacing units) takes 60/5 s = 12 s.
		let word = 31.0 * f.unit_ms() + 19.0 * f.spacing_unit_ms();
		assert!(approx(word, 12_000.0));

		// Effective speed can't exceed character speed.
		assert_eq!(Timing::with_farnsworth(10.0, 30.0).effective_wpm, 10.0);
	}

	#[test]
	fn classify_press_and_gap() {
		let d = KeyDecoder::new(Timing::new(20.0), false); // unit 60 ms
		assert_eq!(d.classify_press(40.0), Symbol::Dit);
		assert_eq!(d.classify_press(60.0), Symbol::Dit);
		assert_eq!(d.classify_press(110.0), Symbol::Dit);
		assert_eq!(d.classify_press(130.0), Symbol::Dah);
		assert_eq!(d.classify_press(180.0), Symbol::Dah);
		assert_eq!(d.classify_press(500.0), Symbol::Dah);

		assert_eq!(d.classify_gap(60.0), Gap::Intra);
		assert_eq!(d.classify_gap(110.0), Gap::Intra);
		assert_eq!(d.classify_gap(180.0), Gap::Letter);
		assert_eq!(d.classify_gap(290.0), Gap::Letter);
		assert_eq!(d.classify_gap(420.0), Gap::Word);
	}

	#[test]
	fn farnsworth_gap_thresholds_stretch() {
		let d = KeyDecoder::new(Timing::with_farnsworth(20.0, 10.0), false);
		// Elements still at 20 WPM.
		assert_eq!(d.classify_press(110.0), Symbol::Dit);
		// A normal 20 WPM letter gap (180 ms) is now too short to end a letter.
		assert_eq!(d.classify_gap(180.0), Gap::Intra);
		let t = d.timing();
		assert_eq!(d.classify_gap(t.letter_gap_ms()), Gap::Letter);
		assert_eq!(d.classify_gap(t.word_gap_ms()), Gap::Word);
	}

	/// Key `text` into the decoder, multiplying each element by a cycling jitter factor.
	fn key_text(dec: &mut KeyDecoder, text: &str, t: Timing, jitter: &[f32]) -> String {
		let mut now = 1000.0f64;
		for (i, e) in to_elements(text, t).iter().enumerate() {
			let ms = (e.ms * jitter[i % jitter.len()]) as f64;
			if e.on {
				dec.key_down(now);
				now += ms;
				dec.key_up(now);
			} else {
				// Simulate the UI polling during silence.
				let end = now + ms;
				while now + 10.0 < end {
					now += 10.0;
					dec.update(now);
				}
				now = end;
			}
		}
		dec.update(now + 10_000.0);
		dec.text().trim_end().to_string()
	}

	#[test]
	fn decoder_exact_timing() {
		let t = Timing::new(20.0);
		let mut d = KeyDecoder::new(t, false);
		assert_eq!(key_text(&mut d, "SOS", t, &[1.0]), "SOS");
		assert_eq!(d.raw(), "... --- ... / ");
		d.clear();
		assert_eq!(key_text(&mut d, "CQ DE K1ABC", t, &[1.0]), "CQ DE K1ABC");
	}

	#[test]
	fn decoder_tolerates_jitter() {
		let t = Timing::new(15.0);
		let mut d = KeyDecoder::new(t, false);
		let jitter = [1.25, 0.8, 1.1, 0.9, 1.3, 0.75];
		assert_eq!(
			key_text(&mut d, "THE QUICK BROWN FOX 73", t, &jitter),
			"THE QUICK BROWN FOX 73"
		);
	}

	#[test]
	fn decoder_with_farnsworth() {
		let t = Timing::with_farnsworth(20.0, 8.0);
		let mut d = KeyDecoder::new(t, false);
		assert_eq!(key_text(&mut d, "PARIS PARIS", t, &[1.0]), "PARIS PARIS");
	}

	#[test]
	fn wordsworth_timing() {
		// 20 WPM characters at 10 WPM effective: a PARIS word takes 6 s.
		let t = Timing::with_wordsworth(20.0, 10.0);
		let std = Timing::new(20.0);
		// Elements and letter gaps unchanged.
		assert!(approx(t.dit_ms(), std.dit_ms()));
		assert!(approx(t.letter_gap_ms(), std.letter_gap_ms()));
		// Only the word gap absorbs the extra time: 6000 - 43 * 60.
		assert!(approx(t.word_gap_ms(), 6000.0 - 43.0 * 60.0));
		let word = total_ms(&to_elements("PARIS", t)) + t.word_gap_ms();
		assert!(approx(word, 6000.0));
		// Equal speeds are identical to standard timing.
		let same = Timing::with_wordsworth(20.0, 20.0);
		assert!(approx(same.word_gap_ms(), std.word_gap_ms()));
	}

	#[test]
	fn farnsworth_and_wordsworth_differ() {
		let f = Timing::with_farnsworth(20.0, 10.0);
		let w = Timing::with_wordsworth(20.0, 10.0);
		assert!(f.letter_gap_ms() > w.letter_gap_ms());
		assert!(w.word_gap_ms() > f.word_gap_ms());
	}

	#[test]
	fn decoder_with_wordsworth() {
		let t = Timing::with_wordsworth(20.0, 8.0);
		let mut d = KeyDecoder::new(t, false);
		assert_eq!(key_text(&mut d, "PARIS PARIS", t, &[1.0]), "PARIS PARIS");
	}

	#[test]
	fn decoder_unknown_sequence() {
		let mut d = KeyDecoder::new(Timing::new(20.0), false);
		let mut t = 0.0;
		for _ in 0..9 {
			d.key_down(t);
			d.key_up(t + 60.0);
			t += 120.0;
		}
		d.update(t + 1000.0);
		assert_eq!(d.text(), format!("{UNKNOWN_CHAR} "));
	}

	#[test]
	fn decoder_streams_live() {
		let mut d = KeyDecoder::new(Timing::new(20.0), false);
		d.key_down(0.0);
		assert!(d.is_key_down());
		d.key_up(60.0);
		assert_eq!(d.current_symbol(), ".");
		assert_eq!(d.text(), "");
		d.update(150.0); // still intra-character
		assert_eq!(d.text(), "");
		d.update(200.0); // letter gap reached
		assert_eq!(d.text(), "E");
		assert_eq!(d.current_symbol(), "");
		d.update(500.0); // word gap reached
		assert_eq!(d.text(), "E ");
		d.update(5000.0); // no repeated spaces
		assert_eq!(d.text(), "E ");
	}

	#[test]
	fn adaptive_follows_slower_operator() {
		// Nominal 20 WPM, but the operator keys at 12 WPM.
		let nominal = Timing::new(20.0);
		let actual = Timing::new(12.0);
		let text = "PARIS PARIS PARIS";

		let mut fixed = KeyDecoder::new(nominal, false);
		assert_ne!(key_text(&mut fixed, text, actual, &[1.0]), text);

		let mut adaptive = KeyDecoder::new(nominal, true);
		assert_eq!(key_text(&mut adaptive, text, actual, &[1.0]), text);
		let wpm = adaptive.estimated_wpm();
		assert!((10.5..13.5).contains(&wpm), "estimated {wpm} WPM");
	}

	#[test]
	fn adaptive_follows_faster_operator() {
		let nominal = Timing::new(12.0);
		let actual = Timing::new(22.0);
		let mut adaptive = KeyDecoder::new(nominal, true);
		let out = key_text(&mut adaptive, "TEST TEST TEST TEST", actual, &[1.0]);
		assert!(out.ends_with("TEST TEST"), "got {out:?}");
		let wpm = adaptive.estimated_wpm();
		assert!((19.0..25.0).contains(&wpm), "estimated {wpm} WPM");
	}
}
