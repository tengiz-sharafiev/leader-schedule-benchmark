use criterion::{Criterion, black_box, criterion_group, criterion_main};
use leader_schedule_bench::LeaderSchedule;

fn leader_schedule_benchmark(c: &mut Criterion) {
    dotenvy::dotenv().ok();
    let rpc_url = std::env::var("SOLANA_RPC_URL").expect("SOLANA_RPC_URL must be set");
    let rpc_client = solana_rpc_client::nonblocking::rpc_client::RpcClient::new(rpc_url);

    let rt = tokio::runtime::Runtime::new().unwrap();

    let epoch = 960u64;
    let schedule = rt
        .block_on(LeaderSchedule::new(epoch, &rpc_client))
        .expect("Failed to fetch leader schedule");

    let validators = schedule.validator_set(50, 0.1);

    let first_slot = schedule.epoch_schedule().get_first_slot_in_epoch(epoch);
    let slots_in_epoch = schedule.epoch_schedule().get_slots_in_epoch(epoch);
    let step = slots_in_epoch / 101;
    let test_slots: Vec<u64> = (1..=100).map(|i| first_slot + step * i).collect();

    let mut group = c.benchmark_group("next_raiku_leader_and_slot");

    group.bench_function("new", |b| {
        b.iter(|| {
            for &slot in &test_slots {
                black_box(schedule.next_leader_and_slot_new(black_box(slot), &validators));
            }
        });
    });

    group.bench_function("old", |b| {
        b.iter(|| {
            for &slot in &test_slots {
                black_box(schedule.next_leader_and_slot_old(black_box(slot), &validators));
            }
        });
    });

    group.finish();
}

criterion_group!(benches, leader_schedule_benchmark);
criterion_main!(benches);
