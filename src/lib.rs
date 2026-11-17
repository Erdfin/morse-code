//! Core, UI-independent logic for the Morse trainer.
//!
//! * [`morse`] – character table, encode/decode, PARIS/Farnsworth timing and a
//!   timing-based straight-key decoder.
//! * [`compare`] – aligns a transcription against the original text.
//! * [`generator`] – random practice text (letters, words, callsigns).

pub mod compare;
pub mod generator;
pub mod morse;
