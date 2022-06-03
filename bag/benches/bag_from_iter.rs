use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn yadf() -> Vec<(u64, String)> {
    let paths = vec![std::path::PathBuf::from("..").canonicalize().unwrap()];
    let items = yadf::Yadf::builder()
        .paths(paths)
        .build()
        .scan::<seahash::SeaHasher>();
    items
        .into_inner()
        .into_iter()
        .flat_map(|(key, group)| {
            group
                .into_iter()
                .map(|p| (key, format!("{p:?}")))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn bench_bag_from_iter_yadf(c: &mut Criterion) {
    let mut group = c.benchmark_group("Bags");
    group.bench_function("IndexBag::from_iter", |b| {
        b.iter_with_setup(yadf, |items| bag::IndexBag::from_iter(items))
    });
    group.bench_function("HashBag::from_iter", |b| {
        b.iter_with_setup(yadf, |items| bag::HashBag::from_iter(items))
    });
    group.bench_function("TreeBag::from_iter", |b| {
        b.iter_with_setup(yadf, |items| bag::TreeBag::from_iter(items))
    });
    group.finish();
}

criterion_group!(benches, bench_bag_from_iter_yadf);
criterion_main!(benches);
