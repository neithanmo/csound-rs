use csound::{ControlChannel, Csound, InputChannel, Myflt};
use std::hint::black_box;
use std::time::{Duration, Instant};

static ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

chn_k "bench_control", 1

instr 1
  gkbench chnget "bench_control"
endin
"#;

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .and_then(|arg| arg.parse::<u64>().ok())
        .unwrap_or(1_000_000);

    let cs = Csound::new().expect("Failed to create Csound instance");
    cs.set_option("-n").expect("Failed to set -n option");
    cs.set_option("-d").expect("Failed to set -d option");
    cs.set_option("-m0").expect("Failed to set -m0 option");
    cs.compile_orc(ORC, 0)
        .expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");
    cs.send_string_event("i1 0 3600", 0)
        .expect("Failed to start instrument");

    let channel_name = "bench_control";
    let writer = cs
        .get_input_channel::<ControlChannel>(channel_name)
        .expect("Failed to get input channel");

    println!("iterations: {}", iterations);
    println!("mode: locked vs unsafe (single-thread)");

    let locked_write_read = bench_locked_write_read(&cs, &writer, iterations);
    print_result(
        "locked write+read (single-thread)",
        iterations,
        locked_write_read,
    );

    let unsafe_write_read = bench_unsafe_write_read(&cs, &writer, iterations);
    print_result(
        "unsafe write+read (single-thread)",
        iterations,
        unsafe_write_read,
    );

}

fn bench_locked_write_read(
    cs: &Csound,
    channel: &InputChannel<'_, ControlChannel>,
    iterations: u64,
) -> Duration {
    let start = Instant::now();
    let mut sum = 0.0;
    for i in 0..iterations {
        let mut guard = channel.lock();
        let value = black_box(i as Myflt);
        guard.set(value);
        sum += guard.get();
        cs.perform_ksmps();
    }
    black_box(sum);
    start.elapsed()
}

fn bench_unsafe_write_read(
    cs: &Csound,
    channel: &InputChannel<'_, ControlChannel>,
    iterations: u64,
) -> Duration {
    let start = Instant::now();
    let mut sum = 0.0;
    for i in 0..iterations {
        let value = black_box(i as Myflt);
        // SAFETY: single-threaded access for benchmarking.
        unsafe {
            channel.set(value);
            sum += channel.get();
        }
        cs.perform_ksmps();
    }
    black_box(sum);
    start.elapsed()
}

fn print_result(label: &str, iterations: u64, elapsed: Duration) {
    let nanos = elapsed.as_nanos() as f64;
    let per_op = nanos / iterations as f64;
    println!("{label}: {elapsed:?} total, {per_op:.2} ns/op");
}
