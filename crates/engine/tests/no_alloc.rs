//! The audio path must never allocate or free: any allocation inside `assert_no_alloc` aborts this
//! test binary (debug build), which makes `cargo test` fail. The guard only exists in debug builds.
#![cfg(debug_assertions)]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use engine::Engine;
use engine::link::{self, Command};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

#[test]
fn ten_seconds_of_audio_without_allocation() {
    // Everything is allocated here, on the "control thread", before audio starts.
    let (mut audio, mut handle) = link::connect(Engine::new(48_000, 1024, 120.0));
    let mut block = vec![0.0_f32; 512 * 2];
    let blocks = 48_000 * 10 / 512;

    for i in 0..blocks {
        // Exercise every command while playing.
        match i % 200 {
            0 => {
                handle.send(Command::Start);
            }
            50 => {
                handle.send(Command::SetTempo(60.0 + (i % 140) as f64));
            }
            100 => {
                handle.send(Command::Metronome(i % 400 == 100));
            }
            150 => {
                handle.send(Command::Stop);
            }
            _ => {}
        }
        // Odd sizes too: the engine slices blocks larger than its bus.
        let frames = if i % 7 == 0 { 333 } else { 512 };
        let buf = &mut block[..frames * 2];
        assert_no_alloc(|| audio.process(buf, 2));
        while handle.next_event().is_some() {}
    }
    assert_eq!(handle.snapshot().nan_count, 0);
}
