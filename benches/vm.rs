use criterion::{criterion_group, criterion_main, Criterion, SamplingMode};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// path to VM binary which should be benchmarked
fn vm() -> String {
    std::env::var("PL0_VM").unwrap_or_else(|_| env!("CARGO_BIN_EXE_pl0_vm_rs").to_string())
}

fn is_external_vm() -> bool {
    std::env::var_os("PL0_VM").is_some()
}

/// (program name, what it stresses, expected last output line)
const PROGRAMS: &[(&str, &str, &str)] = &[
    ("loop", "arithmetic, loads/stores, jumps", "212503"),
    ("primes", "nested loops, mul/div", "1229"),
    ("calls", "procedure call/return", "599998"),
    ("fib", "recursion", "75025"),
    ("output", "printing 200000 lines", "199999"),
];

fn program_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches/programs")
        .join(format!("{name}.cl0"))
}

fn check_output(name: &str, expected_last_line: &str) {
    let out = Command::new(vm())
        .arg(program_path(name))
        .stdin(Stdio::null())
        .output()
        .expect("failed to start VM binary");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let last = stdout.lines().last().unwrap_or("").trim();
    assert_eq!(
        last, expected_last_line,
        "benchmark program '{name}' produced wrong output (stderr: {})",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn run(name: &str) {
    let status = Command::new(vm())
        .arg(program_path(name))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("failed to start VM binary");
    assert!(is_external_vm() || status.success(), "VM exited with {status} on '{name}'");
}

fn bench_programs(c: &mut Criterion) {
    if is_external_vm() {
        eprintln!("benchmarking external VM from PL0_VM: {}", vm());
    }
    let mut group = c.benchmark_group("vm");
    // flat sampling is meant for long-running benchmarks like these
    group.sampling_mode(SamplingMode::Flat);
    for &(name, _what, expected) in PROGRAMS {
        check_output(name, expected);
        group.bench_function(name, |b| b.iter(|| run(name)));
    }
    group.finish();
}

criterion_group! {
    name = benches;
    // the programs should be long-running, so few samples are enough
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(8));
    targets = bench_programs
}
criterion_main!(benches);
