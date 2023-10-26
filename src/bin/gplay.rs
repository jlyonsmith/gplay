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
        eprintln!("{}", Paint::yellow(format!("warning: {}", args)));
    }
    fn error(self: &Self, args: Arguments) {
        eprintln!("{}", Paint::red(format!("error: {}", args)));
    }
}

#[tokio::main]
async fn main() {
    let logger = GplayLogger::new();

    if let Err(error) = GplayTool::new(&logger).run(std::env::args_os()).await {
        error!(logger, "{}", error);
        std::process::exit(1);
    }
}
