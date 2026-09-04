use agent_semantic_provider_transport::byte_text;
use criterion::Criterion;
use criterion::criterion_group;
use criterion::criterion_main;
use std::hint::black_box;

fn transport_hot_path(c: &mut Criterion) {
    let input =
        b"src/capture.rs:42:12:capture_output_stream\nsrc/transport.rs:88:4:run_provider_process\n";
    c.bench_function("transport_hot_path", |b| {
        b.iter(|| {
            let mut checksum = 0usize;
            for record in byte_text::split_lf_or_nul_records(input) {
                checksum = checksum
                    .wrapping_add(record.len())
                    .wrapping_add(byte_text::find_byte(b':', record).unwrap_or(0));
                black_box(byte_text::lowercase_lossy_string(record));
            }
            black_box(checksum);
        });
    });
}

criterion_group!(benches, transport_hot_path);
criterion_main!(benches);
