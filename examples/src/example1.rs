use csound::Csound;

fn main() {
    let cs = Csound::new();

    let args = ["csound", "examples/test1.csd"];
    cs.compile(&args).unwrap();
    cs.perform();
    cs.stop();
}
