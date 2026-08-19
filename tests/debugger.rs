//! Integration tests for the Csound debugger bindings.
//!
//! Structured after Csound's own `csound_debugger_test.cpp`,
//! `csound_debug_callback_test.cpp` and
//! `csound_debug_fsig_globals_arrays_test.cpp`, so the call sequences match
//! what the engine is known to support.

use std::cell::RefCell;
use std::rc::Rc;

use csound::{Csound, MessageType};

static ORC: &str = "instr 1\nasig oscil 1, p4\nendin\n";

fn create_test_csound() -> Csound {
    let cs = Csound::new().expect("Failed to create Csound instance");
    cs.set_option("-n").expect("Failed to set -n option");
    cs.set_option("-d").expect("Failed to set -d option");
    cs.set_option("-m0").expect("Failed to set -m0 option");
    cs.message_string_callback(|_: MessageType, _: &str| {});
    cs
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

#[test]
fn debugger_init_and_clean() {
    let cs = create_test_csound();
    let dbg = cs.debugger().expect("debugger init should succeed");
    drop(dbg); // csoundDebuggerClean runs here
}

#[test]
fn debugger_init_fails_under_parallel_processing() {
    // The engine refuses to debug a multi-threaded performance.
    let cs = create_test_csound();
    cs.set_option("-j 2").expect("failed to set -j 2");
    assert!(
        cs.debugger().is_err(),
        "debugger init must fail when parallel processing is enabled"
    );
}

#[test]
fn debugger_can_be_reinitialised() {
    let cs = create_test_csound();
    {
        let _dbg = cs.debugger().expect("first init");
    }
    let _dbg = cs.debugger().expect("second init after clean");
}

// ---------------------------------------------------------------------------
// Breakpoints
// ---------------------------------------------------------------------------

#[test]
fn breakpoints_can_be_set_and_cleared() {
    let cs = create_test_csound();
    let mut dbg = cs.debugger().expect("debugger init");

    dbg.set_line_breakpoint(3, 0, 0);
    dbg.set_line_breakpoint(5, 1, 0);
    dbg.set_instrument_breakpoint(3.4, 0);
    dbg.set_instrument_breakpoint(1.1, 0);
    dbg.remove_line_breakpoint(3, 0);
    dbg.remove_instrument_breakpoint(3.4);
    dbg.clear_breakpoints();
}

#[test]
fn instrument_breakpoint_fires_once() {
    let mut cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1.1 0 1 440", 0)
        .expect("event failed");

    let hits = Rc::new(RefCell::new(0usize));
    let seen_p1 = Rc::new(RefCell::new(0.0f64));

    let mut dbg = cs.debugger().expect("debugger init");
    {
        let hits = Rc::clone(&hits);
        let seen_p1 = Rc::clone(&seen_p1);
        dbg.on_breakpoint(move |bkpt| {
            *hits.borrow_mut() += 1;
            if let Some(instr) = bkpt.instrument() {
                *seen_p1.borrow_mut() = instr.p1();
            }
        });
    }
    dbg.set_instrument_breakpoint(1.1, 0);

    // Csound stops at the breakpoint and stays stopped, so the count reaches
    // exactly one no matter how many cycles are attempted.
    for _ in 0..1000 {
        cs.perform_ksmps();
    }

    assert_eq!(*hits.borrow(), 1, "breakpoint should fire exactly once");
    assert!(
        (*seen_p1.borrow() - 1.1).abs() < 1e-9,
        "expected to stop in instance 1.1, got {}",
        seen_p1.borrow()
    );
}

#[test]
fn breakpoint_can_resume_and_remove_itself() {
    let mut cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1.1 0 1 440", 0).unwrap();
    cs.send_string_event("i 1.2 0 1 880", 0).unwrap();

    let hits = Rc::new(RefCell::new(0usize));

    let mut dbg = cs.debugger().expect("debugger init");
    {
        let hits = Rc::clone(&hits);
        dbg.on_breakpoint(move |bkpt| {
            *hits.borrow_mut() += 1;
            // One-shot: drop the breakpoint and let the performance continue.
            if let Some(instr) = bkpt.instrument() {
                bkpt.remove_instrument_breakpoint(instr.p1());
            }
            bkpt.continue_();
        });
    }
    dbg.set_instrument_breakpoint(1.1, 0);

    for _ in 0..64 {
        cs.perform_ksmps();
    }

    assert_eq!(
        *hits.borrow(),
        1,
        "a self-removing breakpoint should fire once and then let the score run"
    );
}

#[test]
fn no_breakpoint_means_no_callback() {
    let mut cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 1 440", 0).unwrap();

    let hits = Rc::new(RefCell::new(0usize));
    let mut dbg = cs.debugger().expect("debugger init");
    {
        let hits = Rc::clone(&hits);
        dbg.on_breakpoint(move |_| *hits.borrow_mut() += 1);
    }

    for _ in 0..32 {
        cs.perform_ksmps();
    }
    assert_eq!(*hits.borrow(), 0);
}

// ---------------------------------------------------------------------------
// k-cycle callback
// ---------------------------------------------------------------------------

#[test]
fn k_cycle_callback_fires_every_cycle() {
    let mut cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 2 440", 0).unwrap();

    let count = Rc::new(RefCell::new(0usize));
    let mut dbg = cs.debugger().expect("debugger init");
    {
        let count = Rc::clone(&count);
        dbg.on_k_cycle(move || *count.borrow_mut() += 1);
    }

    const CYCLES: usize = 20;
    for _ in 0..CYCLES {
        cs.perform_ksmps();
    }

    assert_eq!(
        *count.borrow(),
        CYCLES,
        "the k-cycle callback should fire once per control cycle"
    );
}

#[test]
fn k_cycle_callback_can_be_removed() {
    let mut cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 2 440", 0).unwrap();

    let count = Rc::new(RefCell::new(0usize));
    let mut dbg = cs.debugger().expect("debugger init");
    {
        let count = Rc::clone(&count);
        dbg.on_k_cycle(move || *count.borrow_mut() += 1);
    }

    for _ in 0..5 {
        cs.perform_ksmps();
    }
    let after_five = *count.borrow();
    assert_eq!(after_five, 5);

    dbg.remove_k_cycle_callback();
    for _ in 0..5 {
        cs.perform_ksmps();
    }
    assert_eq!(
        *count.borrow(),
        after_five,
        "no further calls after removal"
    );
}

#[test]
fn callback_panic_is_contained() {
    // A panic must not unwind across the FFI boundary; it is caught and logged.
    let mut cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 2 440", 0).unwrap();

    let mut dbg = cs.debugger().expect("debugger init");
    dbg.on_k_cycle(|| panic!("callback panic on purpose"));

    for _ in 0..4 {
        cs.perform_ksmps();
    }
    // Reaching here at all is the assertion: the process survived.
}

// ---------------------------------------------------------------------------
// Inspection
// ---------------------------------------------------------------------------

#[test]
fn instrument_instances_are_listed_during_performance() {
    let mut cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 2 440", 0).unwrap();

    let mut dbg = cs.debugger().expect("debugger init");
    dbg.set_instrument_breakpoint(1.0, 0);

    let mut instances = 0usize;
    for _ in 0..64 {
        cs.perform_ksmps();
        let list = dbg.instr_instances();
        if !list.is_empty() {
            instances = list.len();
            break;
        }
    }

    assert!(instances > 0, "expected at least one active instance");
}

#[test]
fn instrument_variables_are_readable() {
    // A k-rate variable with a known value, so the read can be checked.
    let orc = "instr 1\nkval init 42.5\nasig oscil 1, p4\nendin\n";

    let mut cs = create_test_csound();
    cs.compile_orc(orc, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 2 440", 0).unwrap();

    let mut dbg = cs.debugger().expect("debugger init");
    dbg.set_instrument_breakpoint(1.0, 0);

    let mut found = None;
    for _ in 0..64 {
        cs.perform_ksmps();
        let list = dbg.instr_instances();
        for instr in list.iter() {
            let vars = instr.variables();
            if let Some(var) = vars.get("kval")
                && let Ok(value) = var.scalar()
            {
                found = Some(value);
            }
        }
        if found.is_some() {
            break;
        }
    }

    assert_eq!(
        found,
        Some(42.5),
        "expected to read kval from the stopped instrument"
    );
}

#[test]
fn variable_type_mismatch_is_rejected() {
    let orc = "instr 1\nkval init 1\nasig oscil 1, p4\nendin\n";

    let mut cs = create_test_csound();
    cs.compile_orc(orc, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 2 440", 0).unwrap();

    let mut dbg = cs.debugger().expect("debugger init");
    dbg.set_instrument_breakpoint(1.0, 0);

    for _ in 0..64 {
        cs.perform_ksmps();
        let list = dbg.instr_instances();
        for instr in list.iter() {
            let vars = instr.variables();
            if let Some(var) = vars.get("kval") {
                // kval is k-rate: asking for other representations must fail
                // rather than reinterpret the bytes.
                assert!(var.scalar().is_ok());
                assert!(var.string().is_err());
                assert!(var.array().is_err());
                assert!(var.fsig(0).is_err());
                return;
            }
        }
    }
    panic!("never observed kval");
}

#[test]
fn global_variables_are_listed() {
    let orc = "gkglob init 7\ninstr 1\nasig oscil 1, p4\nendin\n";

    let mut cs = create_test_csound();
    cs.compile_orc(orc, 0).expect("compile failed");
    cs.start().expect("start failed");

    let dbg = cs.debugger().expect("debugger init");
    let globals = dbg.global_variables();

    assert!(!globals.is_empty(), "global pool should not be empty");

    let names: Vec<String> = globals
        .iter()
        .filter_map(|v| v.name().map(str::to_owned))
        .collect();
    assert!(
        names.iter().any(|n| n == "gkglob"),
        "expected gkglob among globals, saw: {names:?}"
    );

    let glob = globals.get("gkglob").expect("gkglob should be present");
    assert_eq!(glob.type_name(), Some("k"));
    assert_eq!(glob.scalar().unwrap(), 7.0);
}

#[test]
fn global_array_serializes() {
    // The writes live in instr 1: k-rate assignments at orchestra level belong
    // to instr 0, which only runs at i-time, so they would leave the array
    // zeroed.
    let orc = "gkarr[] init 4\n\
               instr 1\n\
               gkarr[0] = 1\n\
               gkarr[1] = 2\n\
               gkarr[2] = 3\n\
               gkarr[3] = 4\n\
               asig oscil 1, p4\n\
               endin\n";

    let mut cs = create_test_csound();
    cs.compile_orc(orc, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 1 440", 0).unwrap();
    for _ in 0..4 {
        cs.perform_ksmps();
    }

    let dbg = cs.debugger().expect("debugger init");
    let globals = dbg.global_variables();
    let arr = globals.get("gkarr").expect("gkarr should be present");

    assert_eq!(arr.type_name(), Some("["));
    let (info, data) = arr.array().expect("array serialization failed");
    assert_eq!(info.dimensions, 1);
    assert_eq!(info.element_type, "k");
    assert_eq!(data, vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn udo_frames_are_listed() {
    // A UDO called from instr 1, so there is a frame to find.
    let orc = "opcode Doubler, k, k\n\
               kin xin\n\
               kout = kin * 2\n\
               xout kout\n\
               endop\n\
               instr 1\n\
               kres Doubler 21\n\
               asig oscil 1, p4\n\
               endin\n";

    let mut cs = create_test_csound();
    cs.compile_orc(orc, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 2 440", 0).unwrap();

    let mut dbg = cs.debugger().expect("debugger init");
    dbg.set_instrument_breakpoint(1.0, 0);

    let mut names: Vec<String> = Vec::new();
    for _ in 0..64 {
        cs.perform_ksmps();
        let list = dbg.instr_instances();
        for instr in list.iter() {
            let frames = instr.udo_frames();
            if !frames.is_empty() {
                names = frames
                    .iter()
                    .filter_map(|f| f.name().map(str::to_owned))
                    .collect();
            }
        }
        if !names.is_empty() {
            break;
        }
    }

    assert!(
        names.iter().any(|n| n.contains("Doubler")),
        "expected a Doubler UDO frame, saw: {names:?}"
    );
}

#[test]
fn instances_list_is_empty_when_nothing_is_active() {
    let cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");

    let dbg = cs.debugger().expect("debugger init");
    let list = dbg.instr_instances();
    assert!(list.is_empty());
    assert_eq!(list.len(), 0);
}

#[test]
fn replacing_a_callback_does_not_dangle() {
    // Installing a second callback drops the first. The engine must be pointed
    // at the new closure before the old one is freed, or it briefly holds a
    // dangling pointer. Run under `just asan` to check that directly.
    let mut cs = create_test_csound();
    cs.compile_orc(ORC, 0).expect("compile failed");
    cs.start().expect("start failed");
    cs.send_string_event("i 1 0 2 440", 0).unwrap();

    let first = Rc::new(RefCell::new(0usize));
    let second = Rc::new(RefCell::new(0usize));

    let mut dbg = cs.debugger().expect("debugger init");
    {
        let first = Rc::clone(&first);
        dbg.on_k_cycle(move || *first.borrow_mut() += 1);
    }
    for _ in 0..3 {
        cs.perform_ksmps();
    }

    {
        let second = Rc::clone(&second);
        dbg.on_k_cycle(move || *second.borrow_mut() += 1);
    }
    for _ in 0..3 {
        cs.perform_ksmps();
    }

    assert_eq!(
        *first.borrow(),
        3,
        "first callback ran only before replacement"
    );
    assert_eq!(*second.borrow(), 3, "second callback took over");
}
