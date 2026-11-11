fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", fcc::version());
        return;
    }
    // The command line arrives with the state and cli stage of the port.
    eprintln!("usage: fcc --version");
    std::process::exit(2);
}
