use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct EditInsert {
    pub id: String,
}

#[derive(Deserialize)]
pub struct Bundle {
    #[serde(rename = "versionCode")]
    pub version_code: i32,
    pub sha256: String,
}

#[derive(Deserialize)]
pub struct EditBundlesList {
    pub bundles: Vec<Bundle>,
}

#[derive(Deserialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}

#[derive(Deserialize)]
pub struct ApiError {
    pub message: String,
}

#[derive(Deserialize, Serialize)]
pub struct TracksList {
    pub tracks: Vec<Track>,
}

#[derive(Deserialize, Serialize)]
pub struct Track {
    #[serde(rename = "track")]
    pub name: String,
    pub releases: Vec<Release>,
}

#[derive(Deserialize, Serialize)]
pub struct Release {
    pub status: String,
    #[serde(rename = "versionCodes")]
    pub version_codes: Option<Vec<String>>,
}
