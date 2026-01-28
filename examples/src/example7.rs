/* Example 7 - Communicating continuous values with Csound's Channel System
 * Adapted for Rust by Natanael Mojica <neithanmo@gmail.com>, 2019-01-28
 * from the original C example by Steven Yi <stevenyi@gmail.com>
 * 2013.10.28
 *
 * This example introduces using Csound's Channel System to communicate
 * continuous control data (k-rate) from a host program to Csound. The
 * first thing to note is random_line_create(). It takes in a base value
 * and a range in which to vary randomly.  The reset functions calculates
 * a new random target value (end), a random duration in which to
 * run (dur, expressed as # of audio blocks to last in duration), and
 * calculates the increment value to apply to the current value per audio-block.
 * When the target is met, the random_line_tick() function will call
 * random_line_reset() to update a new target value and duration.
 *
 * In this example, we use two random_line's, one for amplitude and
 * another for frequency.  We start a Csound instrument instance that reads
 * from two channels using the chnget opcode. In turn, we update the values
 * to the channel from the host program. To update the channel,
 * we call the csoundSetControlChannel function on the Csound struct, passing
 * a channel name and value.  Note: The random_line_tick() function not only
 * gets us the current value, but also advances the internal state by the
 * increment and by decrementing the duration.
 */
use csound::*;
use rand;

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
    let mut cs = Csound::new().expect("Failed to create Csound instance");

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

    // Initialize channel values before running Csound
    cs.set_control_channel("amp", amp.tick());
    cs.set_control_channel("freq", freq.tick());

    // Main performance loop - perform one block at a time
    while !cs.perform_ksmps() {
        cs.set_control_channel("amp", amp.tick());
        cs.set_control_channel("freq", freq.tick());
    }
}
