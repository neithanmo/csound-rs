use csound::Csound;

fn main() {
    let cs = Csound::new();

    // Open the system audio driver
    cs.set_option("-odac").unwrap();
    cs.set_option("-d").unwrap();

    let args = ["csound", "examples/test1.csd"];
    cs.compile(&args).unwrap();
    
    cs.start().unwrap();
    
    while !cs.perform_ksmps() {
        println!("Performing ksmps...");
    }
}
