use core::fmt::Arguments;
use gplay::{error, GplayLog, GplayTool};
use yansi::Paint;

struct GplayLogger;

impl GplayLogger {
    fn new() -> GplayLogger {
        GplayLogger {}
    }
}

impl GplayLog for GplayLogger {
    fn output(self: &Self, args: Arguments) {
        println!("{}", args);
    }
    fn warning(self: &Self, args: Arguments) {
        eprintln!("{}", format!("warning: {}", Paint::yellow(args)));
    }
    fn error(self: &Self, args: Arguments) {
        eprintln!("{}", format!("error: {}", Paint::red(args)));
    }
}

fn main() {
    let logger = GplayLogger::new();

    if let Err(error) = GplayTool::new(&logger).run(std::env::args_os()) {
        error!(logger, "{}", error);
        std::process::exit(1);
    }
}
