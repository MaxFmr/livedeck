//! Cost of one audio block, to track regressions as voices are added.
//! Read the result against the budget: 128 frames @ 48 kHz = 2 667 µs.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use engine::Engine;
use engine::drums::DrumVoice;
use engine::link::{self, Command};

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

criterion_group!(benches, process_block, process_block_all_drums);
criterion_main!(benches);
