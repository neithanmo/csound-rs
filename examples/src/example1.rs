use csound::Csound;

fn main() {
    let cs = Csound::new().expect("Failed to create Csound instance");

    // Open the system audio driver
    cs.set_option("-odac").unwrap();

    let args = ["csound", "examples/test1.csd"];
    cs.compile(&args).unwrap();

    cs.start().unwrap();

    while !cs.perform_ksmps() {}
}
