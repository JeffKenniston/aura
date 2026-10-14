use criterion::{black_box, criterion_group, criterion_main, Criterion};
use aura_core::identity::{CapabilityEnforcer, Capability, Svid};

fn bench_cedar_policy_evaluation(c: &mut Criterion) {
    let svid = Svid::new("spiffe://aura.local/host");
    c.bench_function("cedar_policy_eval_trusted", |b| b.iter(|| CapabilityEnforcer::check(black_box(&svid), black_box(Capability::FileSystemWrite))));
}

criterion_group!(benches, bench_cedar_policy_evaluation);
criterion_main!(benches);
