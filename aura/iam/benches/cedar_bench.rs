use criterion::{criterion_group, criterion_main, Criterion};
use aura_iam::{Svid, CapabilityEnforcer, Capability};

fn bench_cedar_policy(c: &mut Criterion) {
    let svid = Svid::new("spiffe://aura.local/host");
    c.bench_function("cedar_policy_eval", |b| {
        b.iter(|| {
            CapabilityEnforcer::check(&svid, Capability::FileSystemWrite).unwrap();
        });
    });
}

criterion_group!(benches, bench_cedar_policy);
criterion_main!(benches);
