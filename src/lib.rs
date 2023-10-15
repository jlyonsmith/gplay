mod log_macros;

use clap::Parser;
use core::fmt::Arguments;
use easy_error::{self, ResultExt};
use gcp_auth::{AuthenticationManager, CustomServiceAccount};
use std::{error::Error, path::PathBuf};

pub trait GplayLog {
    fn output(self: &Self, args: Arguments);
    fn warning(self: &Self, args: Arguments);
    fn error(self: &Self, args: Arguments);
}

pub struct GplayTool<'a> {
    log: &'a dyn GplayLog,
}

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct Cli {
    /// Disable colors in output
    #[arg(long = "no-color", short = 'n', env = "NO_CLI_COLOR")]
    no_color: bool,

    #[arg(short = 'c', long = "cred-file", value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
    credentials_file: PathBuf,
}

const PUBLISHER_SCOPES: &[&str; 1] = &["https://www.googleapis.com/auth/androidpublisher"];

impl<'a> GplayTool<'a> {
    pub fn new(log: &'a dyn GplayLog) -> GplayTool {
        GplayTool { log }
    }

    pub async fn run(
        self: &mut Self,
        args: impl IntoIterator<Item = std::ffi::OsString>,
    ) -> Result<(), Box<dyn Error>> {
        let cli = match Cli::try_parse_from(args) {
            Ok(m) => m,
            Err(err) => {
                output!(self.log, "{}", err.to_string());
                return Ok(());
            }
        };

        let service_account = CustomServiceAccount::from_file(cli.credentials_file)?;
        let authentication_manager = AuthenticationManager::from(service_account);
        let token = authentication_manager.get_token(PUBLISHER_SCOPES).await?;

        println!("{}", token.as_str());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_test() {
        struct TestLogger;

        impl TestLogger {
            fn new() -> TestLogger {
                TestLogger {}
            }
        }

        impl GplayLog for TestLogger {
            fn output(self: &Self, _args: Arguments) {}
            fn warning(self: &Self, _args: Arguments) {}
            fn error(self: &Self, _args: Arguments) {}
        }

        let logger = TestLogger::new();
        let mut tool = GplayTool::new(&logger);
        let args: Vec<std::ffi::OsString> = vec!["".into(), "--help".into()];

        tokio_test::block_on(tool.run(args)).unwrap();
    }
}
