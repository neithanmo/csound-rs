/* Example 6 - Generating Score
 * Author: Steven Yi <stevenyi@gmail.com>
 * 2013.10.28
 *
 * This example continues on from Example 5, rewriting the example using
 * a struct called Note. This example also shows how an array of notes
 * could be used multiple times.  The first loop through we use the notes
 * as-is, and during the second time we generate the notes again with
 * the same properties except we alter the fifth p-field up 4 semitones
 * and offset the start time.
 */

extern crate csound;
use csound::*;
use std::fmt::Write;

extern crate rand;
use rand::Rng;

use std::thread;
use std::sync::{Mutex, Arc};

#[derive(Default, Debug)]
pub struct{
    instr_id:u32,
    start:f64,
    duration:f64,
    amplitude:f64,
    midi_keynum:u32,
}

/* Defining our Csound ORC code within a multiline String */
static ORC: &str = "sr=44100
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
static SCO:&str = "i1 0 1 0.5 8.00";

fn midi2pch(midi_keynum:u32) -> String{
    //let mut retval = write!(&mut retval, "i1 {} .25 .5 8.{:02}", (i as f64)*0.25, i).unwrap()

}

fn generate_example2() -> String{
    let mut retval = String::with_capacity(1024);
    for i in 0..13{
        writeln!(&mut retval, "i1 {} .25 .5 8.{:02}", (i as f64)*0.25, i).unwrap();
    }
    println!("{}", retval);
    retval
}

fn generate_example3() -> String{

    let mut rng = rand::thread_rng();

    let mut retval = String::with_capacity(1024);
    let mut values = [[0f64; 13]; 5];

    /* Populate array */
    for i in 0..13{
        values[0][i] = 1f64;
        values[1][i] = i as f64 * 0.25;
        values[2][i] = 0.25;
        values[3][i] = 0.5;
        values[4][i] = rng.gen_range(0.0, 15.0);
    }

    /* Convert array to to String */
    for i in 0..13{
        writeln!(&mut retval, "i{} {} {}  {} 8.{:02}",
                    values[0][i] as u32, values[1][i], values[2][i], values[3][i], values[4][i] as u32).unwrap();
    }
    println!("{}", retval);
    retval
}

fn main() {

    let mut cs = Csound::new();

    /* Using SetOption() to configure Csound
    Note: use only one commandline flag at a time */
    cs.set_option("-odac").unwrap();

    /* Compile the Csound Orchestra string */
    cs.compile_orc(ORC).unwrap();

    /* Compile the Csound SCO String */
    //cs.read_score(&generate_example3()).unwrap();
    cs.read_score(&generate_example3()).unwrap();

    /* When compiling from strings, this call is necessary
     * before doing any performing */
    cs.start().unwrap();

    /* Create a new thread that will use our performance function and
     * pass in our CSOUND structure. This call is asynchronous and
     * will immediately return back here to continue code execution
     */
     let cs = Arc::new(Mutex::new(cs));
     let cs = Arc::clone(&cs);

    let child = thread::spawn( move || {
        while !cs.lock().unwrap().perform_ksmps() {
            /* pass for now */
        }
    });

    child.join().unwrap();


}
