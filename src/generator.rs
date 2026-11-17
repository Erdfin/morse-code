//! Random practice text. Uses a tiny built-in PRNG so there are no extra
//! dependencies and tests can use fixed seeds.

use std::time::{SystemTime, UNIX_EPOCH};

/// xorshift64* – small, fast, good enough for practice text.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
	pub fn new(seed: u64) -> Self {
		Self(seed.max(1))
	}

	pub fn from_time() -> Self {
		let nanos = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map(|d| d.as_nanos() as u64)
			.unwrap_or(0x9E37_79B9_7F4A_7C15);
		Self::new(nanos ^ 0x9E37_79B9_7F4A_7C15)
	}

	pub fn next_u64(&mut self) -> u64 {
		let mut x = self.0;
		x ^= x >> 12;
		x ^= x << 25;
		x ^= x >> 27;
		self.0 = x;
		x.wrapping_mul(0x2545_F491_4F6C_DD1D)
	}

	/// Uniform-ish value in `0..n` (`n > 0`).
	pub fn below(&mut self, n: usize) -> usize {
		(self.next_u64() % n as u64) as usize
	}

	pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
		&items[self.below(items.len())]
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenKind {
	/// Five-letter groups, A–Z.
	Letters,
	/// Five-character groups of letters, digits and common punctuation.
	Mixed,
	/// Common English words.
	Words,
	/// Callsign-style groups, e.g. `K1ABC`, `DL3XY/P`.
	Callsigns,
}

impl GenKind {
	pub const ALL: [GenKind; 4] = [
		GenKind::Letters,
		GenKind::Mixed,
		GenKind::Words,
		GenKind::Callsigns,
	];

	pub fn label(self) -> &'static str {
		match self {
			GenKind::Letters => "Random letters",
			GenKind::Mixed => "Letters, digits & punctuation",
			GenKind::Words => "Common words",
			GenKind::Callsigns => "Callsigns",
		}
	}
}

const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const MIXED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,?/=";
const DIGITS: &[u8] = b"0123456789";

const WORDS: &[&str] = &[
	"THE", "AND", "FOR", "ARE", "BUT", "NOT", "YOU", "ALL", "ANY", "CAN", "HAD", "HER", "WAS",
	"ONE", "OUR", "OUT", "DAY", "GET", "HAS", "HIM", "HIS", "HOW", "MAN", "NEW", "NOW", "OLD",
	"SEE", "TWO", "WAY", "WHO", "BOY", "DID", "ITS", "LET", "PUT", "SAY", "SHE", "TOO", "USE",
	"THAT", "WITH", "HAVE", "THIS", "WILL", "YOUR", "FROM", "THEY", "KNOW", "WANT", "BEEN", "GOOD",
	"MUCH", "SOME", "TIME", "VERY", "WHEN", "COME", "HERE", "JUST", "LIKE", "LONG", "MAKE", "MANY",
	"MORE", "ONLY", "OVER", "SUCH", "TAKE", "THAN", "THEM", "WELL", "WERE", "WHAT", "ABOUT",
	"AFTER", "AGAIN", "BELOW", "COULD", "EVERY", "FIRST", "FOUND", "GREAT", "HOUSE", "LARGE",
	"LEARN", "NEVER", "OTHER", "PLACE", "PLANT", "POINT", "RIGHT", "SMALL", "SOUND", "SPELL",
	"STILL", "STUDY", "THEIR", "THERE", "THESE", "THING", "THINK", "THREE", "WATER", "WHERE",
	"WHICH", "WORLD", "WOULD", "WRITE", "RADIO", "ANTENNA", "SIGNAL", "WEATHER", "NAME", "QTH",
	"RST", "CQ", "DE", "TNX", "FB", "OM", "73", "88", "UR", "ES", "HR", "WX",
];

fn group(rng: &mut Rng, alphabet: &[u8], len: usize) -> String {
	(0..len).map(|_| *rng.pick(alphabet) as char).collect()
}

fn callsign(rng: &mut Rng) -> String {
	let mut s = String::new();
	// Prefix: 1–2 letters, occasionally digit+letter (e.g. 9A, 4X).
	match rng.below(10) {
		0 => {
			s.push(*rng.pick(DIGITS) as char);
			s.push(*rng.pick(LETTERS) as char);
		}
		1..=5 => s.push(*rng.pick(b"KNWFGMI") as char),
		_ => {
			s.push(*rng.pick(LETTERS) as char);
			s.push(*rng.pick(LETTERS) as char);
		}
	}
	s.push(*rng.pick(DIGITS) as char);
	let suffix_len = 1 + rng.below(3);
	s.push_str(&group(rng, LETTERS, suffix_len));
	if rng.below(8) == 0 {
		s.push_str(rng.pick(&["/P", "/M", "/QRP"]));
	}
	s
}

/// Generate `groups` space-separated groups of the given kind.
pub fn generate(kind: GenKind, groups: usize, rng: &mut Rng) -> String {
	(0..groups.max(1))
		.map(|_| match kind {
			GenKind::Letters => group(rng, LETTERS, 5),
			GenKind::Mixed => group(rng, MIXED, 5),
			GenKind::Words => rng.pick(WORDS).to_string(),
			GenKind::Callsigns => callsign(rng),
		})
		.collect::<Vec<_>>()
		.join(" ")
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::morse;

	#[test]
	fn generates_requested_group_count() {
		let mut rng = Rng::new(42);
		for kind in GenKind::ALL {
			let s = generate(kind, 7, &mut rng);
			assert_eq!(s.split(' ').count(), 7, "{kind:?}: {s}");
		}
	}

	#[test]
	fn everything_generated_is_encodable() {
		let mut rng = Rng::new(7);
		for kind in GenKind::ALL {
			for _ in 0..50 {
				let s = generate(kind, 10, &mut rng);
				for c in s.chars().filter(|c| !c.is_whitespace()) {
					assert!(morse::encode_char(c).is_some(), "{c:?} in {s}");
				}
			}
		}
	}

	#[test]
	fn callsigns_contain_a_digit() {
		let mut rng = Rng::new(1);
		for _ in 0..200 {
			let c = callsign(&mut rng);
			assert!(c.chars().any(|ch| ch.is_ascii_digit()), "{c}");
		}
	}

	#[test]
	fn same_seed_same_output() {
		let a = generate(GenKind::Letters, 5, &mut Rng::new(99));
		let b = generate(GenKind::Letters, 5, &mut Rng::new(99));
		assert_eq!(a, b);
	}
}
