# Morse Trainer

A small desktop app for practising Morse code both ways. Built in Rust with egui/eframe and cpal.

## Build and run

You need a stable Rust toolchain (https://rustup.rs).

```
cargo run --release
```

The binary ends up in `target/release/morse-trainer` (`.exe` on Windows). It's a single file you can copy anywhere.

To run the tests:

```
cargo test
```

On Linux you'll need the ALSA development headers first (`libasound2-dev` on Debian/Ubuntu).

## Using it

**send**: press **dot** or **dash**, by clicking or with the keyboard. Each press beeps for one dot or dash at the configured speed. Presses faster than that are queued, so they still play back to back. Translation uses the config timing, counted from your last press, and it's the same whether that press was a dot or a dash. Wait a dash plus a letter gap (0.36 s at 20 WPM) to start a new letter. Wait a bit longer (0.48 s at 20 WPM) to start a new word. Faster speeds need shorter waits. With Farnsworth or Wordsworth, the waits get longer to match the gaps those settings use. The result appears under *translated*. `#` marks a sequence that isn't a known character.

**receive**: press **play** to hear a few random words. Type what you heard under *heard* and press **check**. Letters you got wrong show up in red. Press **play** again for a new set of words.

## Key bindings

| Key | Action |
| --- | --- |
| J | dot (send) |
| K | dash (send) |
| Ctrl+P | play (receive) |
| Enter | check (receive) |

## Config

Click **config** (bottom left) to open settings in a separate window. You can move it anywhere, and the main window keeps working while it's open. It has:

- **speed**: character speed, 5 to 40 WPM
- **pitch**: 400 to 1000 Hz
- **volume**
- **timing**: standard, Farnsworth or Wordsworth, plus an **effective speed** for the last two
  - *Farnsworth*: characters stay at full speed and the gaps between letters and words get longer (ARRL formula).
  - *Wordsworth*: characters and the gaps between letters stay at full speed, and only the gaps between words get longer.

## Details

The defaults are 18 WPM, standard timing and a 600 Hz tone. PARIS timing: one unit is 1200 / WPM ms, a dit is 1 unit, a dah 3, the gap inside a character 1, between letters 3 and between words 7. Tones fade in and out over 5 ms so they don't click.

| File | Contents |
| --- | --- |
| `src/morse.rs` | Character table, encode/decode, PARIS/Farnsworth/Wordsworth timing, decoders (with tests) |
| `src/compare.rs` | Lines up your copy against the original (with tests) |
| `src/generator.rs` | Random practice text (with tests) |
| `src/audio.rs` | Sine synth with click-free envelope (cpal) |
| `src/app.rs` | The UI |

The Morse logic doesn't depend on the UI or audio. `morse.rs` also has a straight-key decoder with timing classification and adaptive speed tracking. It's tested, but the UI doesn't use it right now.
