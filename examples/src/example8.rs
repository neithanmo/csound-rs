/* Example 8 - More efficient Channel Communications
 * Adapted for Rust by Natanael Mojica <neithanmo@gmail.com>, 2020-03-27
 * from the original C example by Steven Yi <stevenyi@gmail.com>
 * 2013.10.28
 *
 * This example builds on Example 7 by replacing the calls to SetControlChannel
 * with using csoundGetChannelPtr. In the Csound API, using SetControlChannel
 * and GetControlChannel is great for quick work, but ultimately it is slower
 * than pre-fetching the actual channel pointer.  This is because
 * Set/GetControlChannel operates by doing a lookup of the Channel Pointer,
 * then setting or getting the value.  This happens on each call. The
 * alternative is to use csoundGetChannelPtr, which fetches the Channel Pointer
 * and lets you directly set and get the value on the pointer.
 *
 * One thing to note though is that csoundSetControlChannel is protected by
 * spinlocks.  This means that it is safe for multithreading to use.  However,
 * if you are working with your own performance-loop, you can correctly process
 * updates to channels and there will be no problems with multithreading.
 *
 */

use csound::{ControlChannel, Csound};

#[derive(Default)]
pub struct RandomLine {
    dur: i32,
    end: f64,
    increment: f64,
    current_val: f64,
    base: f64,
    range: f64,
}

impl RandomLine {
    /// Creates a RandomLine and initializes values
    pub fn new(base: f64, range: f64) -> RandomLine {
        let mut line = RandomLine {
            base,
            range,
            ..Default::default()
        };
        line.reset();
        line
    }

    /// Resets by calculating new end, dur, and increment values
    fn reset(&mut self) {
        self.dur = (rand::random::<i32>() % 256) + 256;
        self.end = rand::random::<f64>();
        self.increment = (self.end - self.current_val) / (self.dur as f64);
    }

    /// Advances state and returns current value
    fn tick(&mut self) -> f64 {
        let current_value = self.current_val;
        self.dur -= 1;
        if self.dur <= 0 {
            self.reset();
        }
        self.current_val += self.increment;
        self.base + (current_value * self.range)
    }
}

/* Defining our Csound ORC code within a multiline String */
static ORC: &str = "sr=44100
  ksmps=32
  nchnls=2
  0dbfs=1
  instr 1
  kamp chnget \"amp\"
  kfreq chnget \"freq\"
  printk 0.5, kamp
  printk 0.5, kfreq
  aout vco2 kamp, kfreq
  aout moogladder aout, 2000, 0.25
  outs aout, aout
endin";

fn main() {
    let cs = Csound::new().expect("Failed to create Csound instance");

    // Using SetOption() to configure Csound
    // Note: use only one commandline flag at a time
    cs.set_option("-odac").unwrap();

    // Compile the Csound Orchestra string
    cs.compile_orc(ORC, 0).unwrap();

    // Compile the Csound SCO String
    cs.send_string_event("i1 0 60", 0).unwrap();

    // When compiling from strings, this call is necessary before performing
    cs.start().unwrap();

    // Create RandomLines for amplitude and frequency
    let mut amp = RandomLine::new(0.4, 0.2);
    let mut freq = RandomLine::new(400.0, 80.0);

    // Retrieve Channel Pointers from Csound
    let amp_channel = cs.get_input_channel::<ControlChannel>("amp").unwrap();
    let freq_channel = cs.get_input_channel::<ControlChannel>("freq").unwrap();

    // Main performance loop - perform one block at a time
    while !cs.perform_ksmps() {
        amp_channel.lock().write(amp.tick());
        freq_channel.lock().write(freq.tick());
    }
}
