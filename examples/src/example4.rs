/* Example 4 - Using Threads
 * Author: Steven Yi <stevenyi@gmail.com>
 * 2013.10.28
 *
 * In this example, we use the Csound thread functions to run Csound in
 * a separate thread. This is a common scenario where you will run
 * Csound in one thread, and doing other things in another thread
 * (i.e. have a GUI main thread, maybe a worker thread for heavy
 * computations, etc.).
 *
 * The Python example used the CsoundPerformanceThread which is a
 * C++ class that uses the same C functions used in this example.
 * To note, Csound offers thread functions so that the the developer
 * won't have to worry about what thread library is used (i.e. pthreads).
 * Using Csound's thread functions helps make your code more portable
 * between platforms.
 */

extern crate csound;
use csound::Csound;

/* Defining our Csound ORC code within a multiline String */
static ORC: &str = "sr=44100
  ksmps=32
  nchnls=2
  0dbfs=1
  instr 1
  aout vco2 0.5, 440
  outs aout, aout
endin";

/*Defining our Csound SCO code */
static SCO: &str = "i1 0 10";

fn main() {
    let cs = Csound::new().expect("Failed to create Csound instance");

    /* Using SetOption() to configure Csound
    Note: use only one commandline flag at a time */
    cs.set_option("-odac").unwrap();

    /* Compile the Csound Orchestra string */
    cs.compile_orc(ORC, 0).unwrap();

    /* Compile the Csound SCO String */
    cs.send_string_event(SCO, 0).unwrap();

    /* When compiling from strings, this call is necessary
     * before doing any performing */
    cs.start().unwrap();

    /* The following is our main performance loop. We will perform one
     * block of sound at a time and continue to do so while it returns false,
     * which signifies to keep processing.
     */
    while !cs.perform_ksmps() {}
}
