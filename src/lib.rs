mod api_structs;
mod log_macros;

use api_structs::*;
use clap::{Parser, Subcommand};
use core::fmt::Arguments;
use easy_error::{self, ResultExt};
use gcp_auth::{AuthenticationManager, CustomServiceAccount};
use reqwest::{Client, Response};
use serde::Deserialize;
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

    // Can we use PhantomData here?  Check the length of the returned body and return that instead?
    async fn get_response<T: for<'de> Deserialize<'de>>(
        response: Response,
    ) -> Result<T, Box<dyn Error>> {
        let status = response.status();

        if status.is_success() {
            Ok(response.json::<T>().await?)
        } else {
            if let Ok(error) = response.json::<ErrorResponse>().await {
                Err(error.error.message.into())
            } else {
                Err(status.to_string().into())
            }
        }
    }

    async fn get_empty_response(response: Response) -> Result<(), Box<dyn Error>> {
        let status = response.status();

        if status.is_success() {
            Ok(())
        } else {
            if let Ok(error) = response.json::<ErrorResponse>().await {
                Err(error.error.message.into())
            } else {
                Err(status.to_string().into())
            }
        }
    }

    async fn open_edit(
        &self,
        client: &Client,
        token: &str,
        package_name: &str,
    ) -> Result<String, Box<dyn Error>> {
        Ok(Self::get_response::<EditInsert>(
            client
                .post(format!(
                    "{}/{package_name}/edits",
                    Self::EDIT_URL,
                    package_name = package_name
                ))
                .bearer_auth(token)
                .body("{}")
                .send()
                .await?,
        )
        .await?
        .id)
    }

    async fn commit_edit(
        &self,
        client: &Client,
        token: &str,
        package_name: &str,
        edit_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        Self::get_empty_response(
            client
                .post(format!(
                    "{}/{package_name}/edits/{edit_id}:commit",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = edit_id
                ))
                .bearer_auth(token)
                .header("Content-Length", 0)
                .send()
                .await?,
        )
        .await
    }

    async fn delete_edit(
        &self,
        client: &Client,
        token: &str,
        package_name: &str,
        edit_id: &str,
    ) -> Result<(), Box<dyn Error>> {
        Self::get_empty_response(
            client
                .delete(format!(
                    "{}/{package_name}/edits/{edit_id}",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = edit_id
                ))
                .bearer_auth(token)
                .send()
                .await?,
        )
        .await
    }

    async fn list_bundles(&self, token: &str, package_name: &str) -> Result<(), Box<dyn Error>> {
        let client = reqwest::Client::new();
        let edit_id = self.open_edit(&client, token, package_name).await?;
        let edit_bundles_list = Self::get_response::<EditBundlesList>(
            client
                .get(format!(
                    "{}/{package_name}/edits/{edit_id}/bundles",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = edit_id
                ))
                .bearer_auth(token)
                .send()
                .await?,
        )
        .await?;

        for bundle in edit_bundles_list.bundles.iter() {
            output!(
                self.log,
                "Version {} [{}]",
                bundle.version_code,
                bundle.sha256
            );
        }

        self.delete_edit(&client, token, package_name, &edit_id)
            .await?;

        Ok(())
    }

    async fn list_tracks(&self, token: &str, package_name: &str) -> Result<(), Box<dyn Error>> {
        let client = reqwest::Client::new();
        let edit_id = self.open_edit(&client, token, package_name).await?;
        let tracks_list = Self::get_response::<TracksList>(
            client
                .get(format!(
                    "{}/{package_name}/edits/{edit_id}/tracks",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = edit_id
                ))
                .bearer_auth(token)
                .send()
                .await?,
        )
        .await?;

        for track in tracks_list.tracks.iter() {
            output!(self.log, "Track '{}'", track.name);
        }

        self.delete_edit(&client, token, package_name, &edit_id)
            .await?;

        Ok(())
    }

    async fn inner_upload_bundle(
        &self,
        client: &Client,
        token: &str,
        package_name: &str,
        edit_id: &str,
        aab_file: &Path,
        track_name: &str,
        timeout_secs: u64,
    ) -> Result<(), Box<dyn Error>> {
        let byte_buf = std::fs::read(aab_file).context("Unable to read bundle file")?;

        output!(
            self.log,
            "Read bundle file '{}' ({} bytes), uploading...",
            aab_file.to_string_lossy(),
            byte_buf.len()
        );

        let bundle = Self::get_response::<Bundle>(
            client
                .post(format!(
                    "{}/{package_name}/edits/{edit_id}/bundles?uploadType=media",
                    Self::UPLOAD_URL,
                    package_name = package_name,
                    edit_id = edit_id
                ))
                .timeout(Duration::from_secs(timeout_secs))
                .bearer_auth(token)
                .header("Content-Type", "application/octet-stream")
                .header("Content-Length", byte_buf.len())
                .body(byte_buf)
                .send()
                .await?,
        )
        .await?;

        output!(
            self.log,
            "Version {} [{}] uploaded",
            bundle.version_code,
            bundle.sha256
        );

        Self::get_response::<Track>(
            client
                .put(format!(
                    "{}/{package_name}/edits/{edit_id}/tracks/{track_name}",
                    Self::EDIT_URL,
                    package_name = package_name,
                    edit_id = edit_id,
                    track_name = track_name
                ))
                .bearer_auth(token)
                .json(&Track {
                    name: track_name.to_string(),
                    releases: vec![Release {
                        status: "draft".to_string(),
                        version_codes: Some(vec![bundle.version_code.to_string()]),
                    }],
                })
                .send()
                .await?,
        )
        .await?;

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
        let client = reqwest::Client::new();
        let edit_id = self.open_edit(&client, token, package_name).await?;

        let result = self
            .inner_upload_bundle(
                &client,
                token,
                package_name,
                &edit_id,
                aab_file,
                track_name,
                timeout_secs,
            )
            .await;

        if let Ok(_) = result {
            output!(self.log, "Committing upload");
            self.commit_edit(&client, token, package_name, &edit_id)
                .await?;
        } else {
            self.delete_edit(&client, token, package_name, &edit_id)
                .await?;
            // Return the error from the failed upload
            return result;
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
