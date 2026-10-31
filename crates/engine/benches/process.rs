//! Cost of one audio block, to track regressions as voices are added.
//! Read the result against the budget: 128 frames @ 48 kHz = 2 667 µs.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use engine::Engine;
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

criterion_group!(benches, process_block);
criterion_main!(benches);
