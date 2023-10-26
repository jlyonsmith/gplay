mod log_macros;

use clap::{Parser, Subcommand};
use core::fmt::Arguments;
use easy_error::{self, ResultExt};
use gcp_auth::{AuthenticationManager, CustomServiceAccount};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
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
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// Disable colors in output
    #[arg(long = "no-color", env = "NO_CLI_COLOR")]
    no_color: bool,

    /// Google API credentials file
    #[arg(short = 'c', long = "cred-file", value_name = "JSON-FILE", value_hint = clap::ValueHint::FilePath)]
    credentials_file: PathBuf,

    /// Google Play package name
    #[arg(short = 'n', long, value_name = "PACKAGE-NAME")]
    package_name: String,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Lists uploaded bundle versions
    ListBundles,
    /// List available release tracks
    ListTracks,
    /// Upload a new bundle
    Upload {
        /// The bundle file to upload
        #[arg(short = 'b', long = "bundle-file", value_name = "AAB-FILE", value_hint = clap::ValueHint::FilePath)]
        aab_file: PathBuf,
        /// The name of the track to add the bundle too
        #[arg(short = 'n', long = "track-name", value_name = "NAME")]
        track_name: String,
        /// The timeout for the upload in seconds
        #[arg(
            short = 't',
            long = "timeout",
            value_name = "TIMEOUT-SECS",
            default_value = "300"
        )]
        timeout_secs: u64,
    },
}

#[derive(Deserialize)]
struct EditInsertResult {
    id: String,
}

#[derive(Deserialize)]
struct Bundle {
    #[serde(rename = "versionCode")]
    version_code: i32,
    sha256: String,
}

#[derive(Deserialize)]
struct EditBundlesList {
    bundles: Vec<Bundle>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: ApiError,
}

#[derive(Deserialize)]
struct ApiError {
    message: String,
}

#[derive(Deserialize, Serialize)]
struct TracksList {
    tracks: Vec<Track>,
}

#[derive(Deserialize, Serialize)]
struct Track {
    #[serde(rename = "track")]
    name: String,
    releases: Vec<Release>,
}

#[derive(Deserialize, Serialize)]
struct Release {
    status: String,
    #[serde(rename = "versionCodes")]
    version_codes: Vec<String>,
}

impl<'a> GplayTool<'a> {
    const EDIT_URL: &str =
        "https://androidpublisher.googleapis.com/androidpublisher/v3/applications";
    const UPLOAD_URL: &str =
        "https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications";

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

        output!(
            self.log,
            "Requesting OAuth token with Android Publisher scope"
        );

        let service_account = CustomServiceAccount::from_file(cli.credentials_file)?;
        let authentication_manager = AuthenticationManager::from(service_account);
        let token = authentication_manager
            .get_token(&["https://www.googleapis.com/auth/androidpublisher"])
            .await?;

        match &cli.command {
            Some(Commands::ListBundles) => {
                self.list_bundles(token.as_str(), &cli.package_name).await?;
            }
            Some(Commands::ListTracks) => {
                self.list_tracks(token.as_str(), &cli.package_name).await?;
            }
            Some(Commands::Upload {
                aab_file,
                track_name,
                timeout_secs,
            }) => {
                self.upload_bundle(
                    token.as_str(),
                    &cli.package_name,
                    aab_file,
                    track_name,
                    *timeout_secs,
                )
                .await?;
            }
            None => {}
        }

        Ok(())
    }

    async fn list_bundles(&self, token: &str, package_name: &str) -> Result<(), Box<dyn Error>> {
        output!(self.log, "Opening an edit");

        // Get an edit id
        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "{}/{package_name}/edits",
                Self::EDIT_URL,
                package_name = package_name
            ))
            .bearer_auth(token)
            .body("{}")
            .send()
            .await?;

        if response.status().is_success() {
            let id = response.json::<EditInsertResult>().await?.id;
            let response = client
                .get(format!(
                    "{}/{package_name}/edits/{edit_id}/bundles",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = id
                ))
                .bearer_auth(token)
                .send()
                .await?;

            if response.status().is_success() {
                let edit_bundles_list = response.json::<EditBundlesList>().await?;

                for bundle in edit_bundles_list.bundles.iter() {
                    output!(
                        self.log,
                        "Version {} [{}]",
                        bundle.version_code,
                        bundle.sha256
                    );
                }
            } else {
                error!(self.log, "Unable to get list of bundles");
            }

            output!(self.log, "Deleting edit");

            let response = client
                .delete(format!(
                    "{}/{package_name}/edits/{edit_id}",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = id
                ))
                .bearer_auth(token)
                .send()
                .await?;

            if !response.status().is_success() {
                warning!(
                    self.log,
                    "Unable to delete edit: {}",
                    response.status().as_u16()
                )
            }
        } else {
            error!(self.log, "Could not open edit")
        }

        Ok(())
    }

    async fn list_tracks(&self, token: &str, package_name: &str) -> Result<(), Box<dyn Error>> {
        output!(self.log, "Opening an edit");

        // Get an edit id
        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "{}/{package_name}/edits",
                Self::EDIT_URL,
                package_name = package_name
            ))
            .bearer_auth(token)
            .body("{}")
            .send()
            .await?;

        if response.status().is_success() {
            let id = response.json::<EditInsertResult>().await?.id;
            let response = client
                .get(format!(
                    "{}/{package_name}/edits/{edit_id}/tracks",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = id
                ))
                .bearer_auth(token)
                .send()
                .await?;

            if response.status().is_success() {
                let tracks_list = response.json::<TracksList>().await?;

                for track in tracks_list.tracks.iter() {
                    output!(self.log, "Track '{}'", track.name);
                }
            } else {
                error!(self.log, "Unable to get list of tracks");
            }

            output!(self.log, "Deleting edit");

            let response = client
                .delete(format!(
                    "{}/{package_name}/edits/{edit_id}",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = id
                ))
                .bearer_auth(token)
                .send()
                .await?;

            if !response.status().is_success() {
                warning!(
                    self.log,
                    "Unable to delete edit: {}",
                    response.status().as_u16()
                )
            }
        } else {
            error!(self.log, "Could not open edit")
        }

        Ok(())
    }

    async fn upload_bundle(
        &self,
        token: &str,
        package_name: &str,
        aab_file: &Path,
        track_name: &str,
        timeout_secs: u64,
    ) -> Result<(), Box<dyn Error>> {
        output!(self.log, "Opening an edit");

        // Get an edit id
        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "{}/{package_name}/edits",
                Self::EDIT_URL,
                package_name = package_name
            ))
            .bearer_auth(token)
            .body("{}")
            .send()
            .await?;

        if response.status().is_success() {
            let id = response.json::<EditInsertResult>().await?.id;
            let byte_buf = std::fs::read(aab_file).context("Unable to read bundle file")?;

            output!(
                self.log,
                "Read bundle file '{}' ({} bytes)",
                aab_file.to_string_lossy(),
                byte_buf.len()
            );
            output!(self.log, "Uploading bundle");

            let response = client
                .post(format!(
                    "{}/{package_name}/edits/{edit_id}/bundles?uploadType=media",
                    Self::UPLOAD_URL,
                    package_name = package_name,
                    edit_id = id
                ))
                .timeout(Duration::from_secs(timeout_secs))
                .bearer_auth(token)
                .header("Content-Type", "application/octet-stream")
                .header("Content-Length", byte_buf.len())
                .body(byte_buf)
                .send()
                .await?;

            if response.status().is_success() {
                let bundle = response.json::<Bundle>().await?;

                output!(
                    self.log,
                    "Uploaded Version {} [{}]",
                    bundle.version_code,
                    bundle.sha256
                );

                let response = client
                    .put(format!(
                        "{}/{package_name}/edits/{edit_id}/tracks/{track_name}",
                        Self::EDIT_URL,
                        package_name = package_name,
                        edit_id = id,
                        track_name = track_name
                    ))
                    .bearer_auth(token)
                    .json(&Track {
                        name: track_name.to_string(),
                        releases: vec![Release {
                            status: "draft".to_string(),
                            version_codes: vec![bundle.version_code.to_string()],
                        }],
                    })
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status_code = response.status().to_string();

                    error!(
                        self.log,
                        "Unable to set track: {}",
                        if let Ok(error) = response.json::<ErrorResponse>().await {
                            error.error.message
                        } else {
                            status_code
                        }
                    );
                }

                let response = client
                    .post(format!(
                        "{}/{package_name}/edits/{edit_id}:commit",
                        Self::EDIT_URL,
                        package_name = package_name,
                        edit_id = id
                    ))
                    .bearer_auth(token)
                    .header("Content-Length", 0)
                    .send()
                    .await?;

                if response.status().is_success() {
                    output!(self.log, "Committed edit")
                } else {
                    let status_code = response.status().to_string();

                    error!(
                        self.log,
                        "Failed to commit edit: {}",
                        if let Ok(error) = response.json::<ErrorResponse>().await {
                            error.error.message
                        } else {
                            status_code
                        }
                    );
                }
            } else {
                error!(
                    self.log,
                    "Unable to upload bundle file: {}",
                    response.status().as_u16()
                );

                let response = client
                    .delete(format!(
                        "{}/{package_name}/edits/{edit_id}:commit",
                        Self::EDIT_URL,
                        package_name = package_name,
                        edit_id = id
                    ))
                    .bearer_auth(token)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    warning!(
                        self.log,
                        "Unable to delete edit: {}",
                        response.status().as_u16()
                    )
                }
            }
        } else {
            error!(self.log, "Could not open edit")
        }

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
