// SPDX-FileCopyrightText: 2023 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use criterion::{criterion_group, criterion_main, Criterion};
use gpiosim::{Level, Simpleton};

// determine overhead from toggling sim lines
fn set_pull(c: &mut Criterion) {
    let s = Simpleton::new(10);
    let offset = 1;

    let mut pull = Level::High;

    c.bench_function("set_pull", |b| {
        b.iter(|| {
            s.set_pull(offset, pull).unwrap();
            pull = match pull {
                Level::High => Level::Low,
                Level::Low => Level::High,
            };
        })
    });
}

criterion_group!(benches, set_pull);
criterion_main!(benches);