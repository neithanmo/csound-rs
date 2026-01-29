/* Example 9 - More efficient Channel Communications
 * Adapted for Rust by Natanael Mojica <neithanmo@gmail.com>, 2020-03-27
 * from the original C example by Steven Yi <stevenyi@gmail.com>
 * 2013.10.28
 *
 * This example continues on from Example 8 and just refactors the
 * creation and setup of Csound Channels into a create_channel()
 * function.  This example illustrates some natural progression that
 * might occur in your own API-based projects, and how you might
 * simplify your own code.
 *
 */

use csound::{ControlChannel, Csound, InputChannel};

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

fn create_channel<'a>(csound: &'a Csound, channel_name: &str) -> InputChannel<'a, ControlChannel> {
    csound
        .get_input_channel::<ControlChannel>(channel_name)
        .expect("Channel does not exist")
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
    let amp_channel = create_channel(&cs, "amp");
    let freq_channel = create_channel(&cs, "freq");

    // Main performance loop - perform one block at a time
    while !cs.perform_ksmps() {
        amp_channel.write(amp.tick());
        freq_channel.write(freq.tick());
    }
}
