use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion,
};
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

fn bench_bag<T>(group: &mut BenchmarkGroup<WallTime>, items: &[(u64, String)])
where
    T: FromIterator<(u64, String)>,
{
    group.bench_function(std::any::type_name::<T>(), |b| {
        b.iter_with_setup(|| items.to_vec(), |items| T::from_iter(black_box(items)))
    });
}

fn bench_bag_from_iter_yadf(c: &mut Criterion) {
    let mut group = c.benchmark_group("FromIterator");
    let items = yadf();
    bench_bag::<bag::IndexBag<_, _>>(&mut group, &items);
    bench_bag::<bag::HashBag<_, _>>(&mut group, &items);
    bench_bag::<bag::TreeBag<_, _>>(&mut group, &items);
    group.finish();
}

criterion_group!(benches, bench_bag_from_iter_yadf);
criterion_main!(benches);
