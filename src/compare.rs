//! Compare a transcription with the original text using a minimum-edit
//! (Levenshtein) alignment, so a single dropped letter doesn't mark the rest
//! of the message as wrong.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffOp {
	/// Copied correctly.
	Match(char),
	/// Copied as the wrong character.
	Wrong { expected: char, got: char },
	/// In the original but not in the transcription.
	Missing(char),
	/// In the transcription but not in the original.
	Extra(char),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
	pub ops: Vec<DiffOp>,
	/// Number of characters in the (normalized) original.
	pub expected_len: usize,
}

impl Comparison {
	pub fn correct(&self) -> usize {
		self.count(|op| matches!(op, DiffOp::Match(_)))
	}
	pub fn wrong(&self) -> usize {
		self.count(|op| matches!(op, DiffOp::Wrong { .. }))
	}
	pub fn missing(&self) -> usize {
		self.count(|op| matches!(op, DiffOp::Missing(_)))
	}
	pub fn extra(&self) -> usize {
		self.count(|op| matches!(op, DiffOp::Extra(_)))
	}
	fn count(&self, f: impl Fn(&DiffOp) -> bool) -> usize {
		self.ops.iter().filter(|op| f(op)).count()
	}

	/// Percentage of the original copied correctly, penalising extra characters.
	pub fn accuracy_percent(&self) -> f32 {
		if self.expected_len == 0 {
			return if self.ops.is_empty() { 100.0 } else { 0.0 };
		}
		let score = self.correct().saturating_sub(self.extra()) as f32;
		100.0 * score / self.expected_len as f32
	}
}

/// Uppercase and collapse whitespace runs to a single space (trimmed).
pub fn normalize(s: &str) -> Vec<char> {
	s.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
		.to_uppercase()
		.chars()
		.collect()
}

pub fn compare(expected: &str, got: &str) -> Comparison {
	let a = normalize(expected);
	let b = normalize(got);
	let (n, m) = (a.len(), b.len());

	let mut d = vec![vec![0u32; m + 1]; n + 1];
	for (i, row) in d.iter_mut().enumerate() {
		row[0] = i as u32;
	}
	for (j, cell) in d[0].iter_mut().enumerate() {
		*cell = j as u32;
	}
	for i in 1..=n {
		for j in 1..=m {
			let cost = u32::from(a[i - 1] != b[j - 1]);
			d[i][j] = (d[i - 1][j - 1] + cost)
				.min(d[i - 1][j] + 1)
				.min(d[i][j - 1] + 1);
		}
	}

	let mut ops = Vec::with_capacity(n.max(m));
	let (mut i, mut j) = (n, m);
	while i > 0 || j > 0 {
		if i > 0 && j > 0 && d[i][j] == d[i - 1][j - 1] + u32::from(a[i - 1] != b[j - 1]) {
			ops.push(if a[i - 1] == b[j - 1] {
				DiffOp::Match(a[i - 1])
			} else {
				DiffOp::Wrong {
					expected: a[i - 1],
					got: b[j - 1],
				}
			});
			i -= 1;
			j -= 1;
		} else if i > 0 && d[i][j] == d[i - 1][j] + 1 {
			ops.push(DiffOp::Missing(a[i - 1]));
			i -= 1;
		} else {
			ops.push(DiffOp::Extra(b[j - 1]));
			j -= 1;
		}
	}
	ops.reverse();
	Comparison {
		ops,
		expected_len: n,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn identical_ignores_case_and_spacing() {
		let c = compare("Hello  World", "  hello world ");
		assert_eq!(c.correct(), 11);
		assert_eq!(c.accuracy_percent(), 100.0);
	}

	#[test]
	fn missing_letter_does_not_shift_the_rest() {
		let c = compare("HELLO WORLD", "HELO WORLD");
		assert_eq!(c.missing(), 1);
		assert_eq!(c.wrong(), 0);
		assert_eq!(c.correct(), 10);
	}

	#[test]
	fn wrong_and_extra() {
		let c = compare("CQ DE", "CQ DX");
		assert_eq!(
			c.ops.last(),
			Some(&DiffOp::Wrong {
				expected: 'E',
				got: 'X'
			})
		);
		let c = compare("ABC", "ABXC");
		assert_eq!(c.extra(), 1);
		assert_eq!(c.correct(), 3);
		assert!(c.accuracy_percent() < 100.0);
	}

	#[test]
	fn empty_inputs() {
		assert_eq!(compare("", "").accuracy_percent(), 100.0);
		assert_eq!(compare("ABC", "").missing(), 3);
		assert_eq!(compare("ABC", "").accuracy_percent(), 0.0);
		assert_eq!(compare("", "X").accuracy_percent(), 0.0);
	}
}
