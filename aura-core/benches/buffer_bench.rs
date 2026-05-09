use aura_core::buffer_pool::BufferPool;
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_buffer_pool(c: &mut Criterion) {
    let pool = BufferPool::new(1024 * 1024, 100);

    c.bench_function("buffer_pool_acquire_release", |b| {
        b.iter(|| {
            let buf = pool.acquire();
            pool.release(buf);
        })
    });
}

fn bench_raw_allocation(c: &mut Criterion) {
    c.bench_function("raw_vec_allocation", |b| {
        b.iter(|| {
            let _buf = vec![0u8; 1024 * 1024];
        })
    });
}

criterion_group!(benches, bench_buffer_pool, bench_raw_allocation);
criterion_main!(benches);
