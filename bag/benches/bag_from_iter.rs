use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::prelude::SliceRandom;

fn yadf() -> Vec<(u64, String)> {
    let paths = vec![std::path::PathBuf::from("..").canonicalize().unwrap()];
    let items = yadf::Yadf::builder()
        .paths(paths)
        .build()
        .scan::<seahash::SeaHasher>();
    let mut items: Vec<_> = items
        .into_inner()
        .into_iter()
        .flat_map(|(key, group)| {
            group
                .into_iter()
                .map(|p| (key, format!("{p:?}")))
                .collect::<Vec<_>>()
        })
        .collect();
    items.shuffle(&mut rand::thread_rng());
    items
}

fn bench_bag_from_iter_yadf(c: &mut Criterion) {
    let mut group = c.benchmark_group("FromIterator");
    let items = yadf();
    group.bench_function("IndexBag", |b| {
        b.iter_with_setup(
            || items.clone(),
            |items| bag::IndexBag::from_iter(black_box(items)),
        )
    });
    group.bench_function("HashBag", |b| {
        b.iter_with_setup(
            || items.clone(),
            |items| bag::HashBag::from_iter(black_box(items)),
        )
    });
    group.bench_function("TreeBag", |b| {
        b.iter_with_setup(
            || items.clone(),
            |items| bag::TreeBag::from_iter(black_box(items)),
        )
    });
    group.finish();
}

criterion_group!(benches, bench_bag_from_iter_yadf);
criterion_main!(benches);
