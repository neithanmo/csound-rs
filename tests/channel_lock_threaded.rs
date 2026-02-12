//! Integration test for threaded channel access with ChannelLock.
//!
//! Verifies that InputChannel and OutputChannel can be moved into
//! separate threads and that ChannelLock provides correct synchronized
//! access to control channel pointers while Csound runs perform_ksmps
//! on another thread.

use csound::{ControlChannel, Csound, InputChannel, MessageType, OutputChannel};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

static ORC: &str = r#"
sr = 44100
ksmps = 32
nchnls = 2
0dbfs = 1

chn_k "gain", 1
chn_k "meter", 2

instr 1
  k_gain chnget "gain"
  k_meter = k_gain * 2
  chnset k_meter, "meter"
endin
"#;

struct SendCsound(std::ptr::NonNull<Csound>);

// SAFETY: The pointer targets a leaked Csound instance that outlives
// all threads.  perform_ksmps() is called only from one thread;
// channel access on other threads is synchronized via ChannelLock.
unsafe impl Send for SendCsound {}

impl SendCsound {
    fn from_ref(cs: &'static Csound) -> Self {
        SendCsound(std::ptr::NonNull::from(cs))
    }

    fn csound(&self) -> &Csound {
        // SAFETY: pointer is valid for the lifetime of the process
        unsafe { self.0.as_ref() }
    }
}

fn create_test_csound() -> Csound {
    let cs = Csound::new().expect("Failed to create Csound instance");
    cs.set_option("-n").expect("Failed to set -n option");
    cs.set_option("-d").expect("Failed to set -d option");
    cs.set_option("-m0").expect("Failed to set -m0 option");
    cs.message_string_callback(|_: MessageType, _: &str| {});
    cs
}

#[test]
fn test_threaded_control_channel_lock_roundtrip() {
    let cs: &'static Csound = Box::leak(Box::new(create_test_csound()));

    cs.compile_orc(ORC, 0).expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");
    cs.send_string_event("i1 0 10", 0)
        .expect("Failed to start instrument");

    let gain_in: InputChannel<'static, ControlChannel> = cs
        .get_input_channel::<ControlChannel>("gain")
        .expect("Failed to get input channel 'gain'");

    let meter_out: OutputChannel<'static, ControlChannel> = cs
        .get_output_channel::<ControlChannel>("meter")
        .expect("Failed to get output channel 'meter'");

    let running = Arc::new(AtomicBool::new(true));

    let perf_cs = SendCsound::from_ref(cs);
    let perf_running = Arc::clone(&running);
    let perf = thread::spawn(move || {
        while perf_running.load(Ordering::SeqCst) {
            if perf_cs.csound().perform_ksmps() {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
    });

    let writer = thread::spawn(move || {
        for i in 0..50 {
            let value = i as f64 / 50.0;
            gain_in.with_lock(move |mut guard| guard.write(value));
            thread::sleep(Duration::from_millis(5));
        }
    });

    let reader = thread::spawn(move || {
        let mut seen_nonzero = false;
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            let value = meter_out.with_lock(|g| g.read());
            if value.abs() > 1e-6 {
                seen_nonzero = true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        seen_nonzero
    });

    writer.join().expect("writer thread panicked");
    let seen_nonzero = reader.join().expect("reader thread panicked");
    running.store(false, Ordering::SeqCst);
    perf.join().expect("performance thread panicked");

    assert!(
        seen_nonzero,
        "Reader should have observed a non-zero meter value via ChannelLock"
    );
}

#[test]
fn test_channel_lock_write_read_consistency() {
    let cs: &'static Csound = Box::leak(Box::new(create_test_csound()));

    cs.compile_orc(ORC, 0).expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");
    cs.send_string_event("i1 0 10", 0)
        .expect("Failed to start instrument");

    let gain_in: InputChannel<'static, ControlChannel> = cs
        .get_input_channel::<ControlChannel>("gain")
        .expect("Failed to get input channel 'gain'");

    let meter_out: OutputChannel<'static, ControlChannel> = cs
        .get_output_channel::<ControlChannel>("meter")
        .expect("Failed to get output channel 'meter'");

    let running = Arc::new(AtomicBool::new(true));

    let perf_cs = SendCsound::from_ref(cs);
    let perf_running = Arc::clone(&running);
    let perf = thread::spawn(move || {
        while perf_running.load(Ordering::SeqCst) {
            if perf_cs.csound().perform_ksmps() {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
    });

    // Write a known value and give Csound time to process
    {
        let mut guard = gain_in.lock();
        guard.write(0.25);
    }
    thread::sleep(Duration::from_millis(100));

    // The orchestra doubles gain -> meter, so meter should be ~0.5
    let meter = meter_out.with_lock(|g| g.read());

    running.store(false, Ordering::SeqCst);
    perf.join().expect("performance thread panicked");

    assert!(
        (meter - 0.5).abs() < 0.01,
        "meter should be 2 * gain (0.5), got {meter}"
    );
}

#[test]
fn test_channel_lock_write_read_consistency_unsafe() {
    let cs: &'static Csound = Box::leak(Box::new(create_test_csound()));

    cs.compile_orc(ORC, 0).expect("Failed to compile orchestra");
    cs.start().expect("Failed to start Csound");
    cs.send_string_event("i1 0 10", 0)
        .expect("Failed to start instrument");

    let gain_in: InputChannel<'static, ControlChannel> = cs
        .get_input_channel::<ControlChannel>("gain")
        .expect("Failed to get input channel 'gain'");

    let meter_out: OutputChannel<'static, ControlChannel> = cs
        .get_output_channel::<ControlChannel>("meter")
        .expect("Failed to get output channel 'meter'");

    let running = Arc::new(AtomicBool::new(true));

    let perf_cs = SendCsound::from_ref(cs);
    let perf_running = Arc::clone(&running);
    let perf = thread::spawn(move || {
        while perf_running.load(Ordering::SeqCst) {
            if perf_cs.csound().perform_ksmps() {
                break;
            }
            thread::sleep(Duration::from_millis(1));
        }
    });

    // Write using the unsafe set (no lock)
    unsafe { gain_in.write(0.25) };
    thread::sleep(Duration::from_millis(100));

    // Read using the unsafe read (no lock)
    let meter = unsafe { meter_out.read() };

    running.store(false, Ordering::SeqCst);
    perf.join().expect("performance thread panicked");

    assert!(
        (meter - 0.5).abs() < 0.01,
        "meter should be 2 * gain (0.5), got {meter}"
    );
}
