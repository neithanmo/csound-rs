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
use csound::*;
use std::fmt::Write;

/*extern crate rand;
use rand::*;*/

use std::thread;
use std::sync::{Mutex, Arc};

/* Defining our Csound ORC code within a multiline String */
static orc: &str = "sr=44100
  ksmps=32
  nchnls=2
  0dbfs=1
  instr 1
  ipch = cps2pch(p5, 12)
  kenv linsegr 0, .05, 1, .05, .7, .4, 0
  aout vco2 p4 * kenv, ipch
  aout moogladder aout, 2000, 0.25
  outs aout, aout
endin";

/* Example 1 - Static Score */
static sco: &str = "i1 0 1 0.5 8.00";

fn generate_example2() -> String{
    let mut retval = String::with_capacity(1024);
    for i in 0..13{
        // nt n = sprintf(note_string, "i1 %g .25 .5 8.%02d\n", i * .25, i);
        writeln!(&mut retval, "i1 {} .25 .5 8.{:.2}", (i as f64)*0.25, i).unwrap();
    }
    retval
}

fn main() {


    let cs = Arc::new(Mutex::new(Csound::new()));
    let cs = Arc::clone(&cs);

    /* Using SetOption() to configure Csound
    Note: use only one commandline flag at a time */
    cs.lock().unwrap().set_option("-odac");

    /* Compile the Csound Orchestra string */
    cs.lock().unwrap().compile_orc(orc).unwrap();

    /* Compile the Csound SCO String */
    let sco_gen2 = generate_example2();
    cs.lock().unwrap().read_score(&sco_gen2).unwrap();

    /* When compiling from strings, this call is necessary
     * before doing any performing */
    cs.lock().unwrap().start().unwrap();
    println!("{}", &generate_example2());

    /* Create a new thread that will use our performance function and
     * pass in our CSOUND structure. This call is asynchronous and
     * will immediately return back here to continue code execution
     */
    let child = thread::spawn( move || {
        while !cs.lock().unwrap().perform_ksmps() {
            /* pass for now */
        }
    });

    child.join().unwrap();


}
