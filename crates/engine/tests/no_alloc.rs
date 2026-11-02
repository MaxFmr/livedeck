//! The audio path must never allocate or free: any allocation inside `assert_no_alloc` aborts this
//! test binary (debug build), which makes `cargo test` fail. The guard only exists in debug builds.
#![cfg(debug_assertions)]

use assert_no_alloc::{AllocDisabler, assert_no_alloc};
use engine::drums::DrumVoice;
use engine::link::{self, BlockTime, Command};
use engine::repeat::Division;
use engine::{Engine, Param};

#[global_allocator]
static ALLOCATOR: AllocDisabler = AllocDisabler;

#[test]
fn ten_seconds_of_audio_without_allocation() {
    // Everything is allocated here, on the "control thread", before audio starts.
    let mut engine = Engine::new(48_000, 1024, 120.0);
    // A sample kit at another rate, so every voice plays resampled.
    let mut bank = engine::sampler::SampleBank {
        sample_rate: 44_100,
        ..Default::default()
    };
    for (i, voice) in bank.voices.iter_mut().enumerate() {
        *voice = (0..4_000 + i * 1_000)
            .map(|n| (n as f32 * 0.05).sin())
            .collect();
    }
    engine.set_sample_bank(bank);
    let (mut audio, mut handle) = link::connect(engine);
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
            120 => {
                handle.send(Command::SetParam(Param::MasterGain, 0.5));
            }
            60 => {
                handle.send(Command::QueuePattern((i / 200 % 4) as u8));
                handle.send(Command::SetSwing(0.5 + (i % 50) as f32 / 200.0));
                handle.send(Command::QueueBassLine((i / 300 % 4) as u8));
                handle.send(Command::QueueStyle((i / 400 % 6) as u8));
                handle.send(Command::UseSamples(i / 200 % 2 == 0));
                handle.send(Command::Latch(i / 300 % 2 == 0));
                handle.send(Command::Voicing {
                    inversion: (i / 100 % 7) as i8 - 3,
                    octave: (i / 100 % 3) as i8 - 1,
                    extension: (i / 100 % 4) as u8,
                });
                handle.send(Command::SetScale((i / 200 % 6) as u8));
                for pad in 0..8 {
                    handle.send(Command::Chord {
                        pad,
                        on: (i / 100 + usize::from(pad)) % 3 != 0,
                    });
                }
                handle.send(Command::SetParam(
                    Param::Brightness,
                    (i % 400) as f32 / 400.0,
                ));
                handle.send(Command::SetParam(Param::MelodyLevel, 0.7));
                handle.send(Command::SetTonic((i / 100 % 12) as u8));
                handle.send(Command::BassMute(i % 1_000 == 60));
                handle.send(Command::SetParam(
                    Param::BassCutoff,
                    (i % 300) as f32 / 300.0,
                ));
            }
            70 => {
                handle.send(Command::SetMutes((i / 200 % 64) as u8));
                handle.send(Command::SetDensity((i % 200) as f32 / 200.0));
                handle.send(Command::Fill((i / 200 % 4) as u8));
                handle.send(Command::BeatRepeat(Division::from_denominator(
                    [4, 8, 16, 32][i / 200 % 4],
                )));
            }
            90 => {
                handle.send(Command::BeatRepeat(Some(Division::Sixteenth)));
                handle.send(Command::BeatRepeatBack {
                    released: Division::Sixteenth,
                    back: Division::Quarter,
                });
            }
            110 => {
                handle.send(Command::BeatRepeat(None));
            }
            170 => {
                for voice in DrumVoice::ALL {
                    handle.send(Command::Drum {
                        voice,
                        velocity: 1.0,
                        gesture_us: i as u64,
                    });
                }
            }
            _ => {}
        }
        // Odd sizes too: the engine slices blocks larger than its bus.
        let frames = if i % 7 == 0 { 333 } else { 512 };
        let buf = &mut block[..frames * 2];
        let time = BlockTime {
            called_us: i as u64,
            callback_us: i as u64,
            playback_us: i as u64 + 5_000,
        };
        assert_no_alloc(|| audio.process_at(buf, 2, time));
        while handle.next_event().is_some() {}
    }
    assert_eq!(handle.snapshot().nan_count, 0);
}
