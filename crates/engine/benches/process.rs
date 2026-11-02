//! Cost of one audio block, to track regressions as voices are added.
//! Read the result against the budget: 128 frames @ 48 kHz = 2 667 µs.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use engine::Engine;
use engine::drums::DrumVoice;
use engine::link::{self, Command};
use engine::repeat::Division;

fn process_block(c: &mut Criterion) {
    let (mut audio, mut handle) = link::connect(Engine::new(48_000, 1024, 120.0));
    handle.send(Command::Start);
    let mut buf = vec![0.0_f32; 128 * 2];
    c.bench_function("process 128 frames stereo (metronome)", |b| {
        b.iter(|| {
            audio.process(black_box(&mut buf), 2);
            while handle.next_event().is_some() {}
        });
    });
}

/// Worst case of the kit: all six voices sounding, retriggered every 8 blocks (~21 ms).
fn process_block_all_drums(c: &mut Criterion) {
    let mut engine = Engine::new(48_000, 1024, 120.0);
    engine.start();
    let mut buf = vec![0.0_f32; 128 * 2];
    let mut n = 0_u32;
    c.bench_function("process 128 frames stereo (6 drum voices)", |b| {
        b.iter(|| {
            if n.is_multiple_of(8) {
                for voice in DrumVoice::ALL {
                    engine.trigger_drum(voice, 1.0);
                }
            }
            n = n.wrapping_add(1);
            engine.process(black_box(&mut buf), 2);
        });
    });
}

/// Same, with the drum bus looped by the beat-repeat (1/32) and a fill playing.
fn process_block_all_drums_repeat(c: &mut Criterion) {
    let mut engine = Engine::new(48_000, 1024, 120.0);
    engine.start();
    engine.beat_repeat(Some(Division::ThirtySecond));
    let mut buf = vec![0.0_f32; 128 * 2];
    let mut n = 0_u32;
    c.bench_function("process 128 frames stereo (6 voices + beat-repeat)", |b| {
        b.iter(|| {
            if n.is_multiple_of(8) {
                for voice in DrumVoice::ALL {
                    engine.trigger_drum(voice, 1.0);
                }
            }
            if n.is_multiple_of(750) {
                engine.queue_fill(0);
            }
            n = n.wrapping_add(1);
            engine.process(black_box(&mut buf), 2);
        });
    });
}

/// The six voices from samples (44.1 kHz, so resampled), each retriggered every 8 blocks.
fn process_block_all_samples(c: &mut Criterion) {
    let mut engine = Engine::new(48_000, 1024, 120.0);
    let mut bank = engine::sampler::SampleBank {
        sample_rate: 44_100,
        ..Default::default()
    };
    for voice in &mut bank.voices {
        *voice = (0..44_100).map(|n| (n as f32 * 0.01).sin()).collect();
    }
    engine.set_sample_bank(bank);
    engine.use_samples(true);
    engine.start();
    let mut buf = vec![0.0_f32; 128 * 2];
    let mut n = 0_u32;
    c.bench_function("process 128 frames stereo (6 sample voices)", |b| {
        b.iter(|| {
            if n.is_multiple_of(8) {
                for voice in DrumVoice::ALL {
                    engine.trigger_drum(voice, 1.0);
                }
            }
            n = n.wrapping_add(1);
            engine.process(black_box(&mut buf), 2);
        });
    });
}

/// Worst case of the melody deck: the 16 synth voices sounding (4 chords + stolen voices), with
/// the drums of the 6-voice case.
fn process_block_full_synth(c: &mut Criterion) {
    let mut engine = Engine::new(48_000, 1024, 120.0);
    engine.start();
    for _ in 0..2 {
        for pad in 0..4 {
            engine.chord(pad, true);
        }
    }
    let mut buf = vec![0.0_f32; 128 * 2];
    let mut n = 0_u32;
    c.bench_function("process 128 frames stereo (16 synth voices + drums)", |b| {
        b.iter(|| {
            if n.is_multiple_of(8) {
                for voice in DrumVoice::ALL {
                    engine.trigger_drum(voice, 1.0);
                }
            }
            n = n.wrapping_add(1);
            engine.process(black_box(&mut buf), 2);
        });
    });
}

criterion_group!(
    benches,
    process_block,
    process_block_all_drums,
    process_block_all_drums_repeat,
    process_block_all_samples,
    process_block_full_synth
);
criterion_main!(benches);
