use semver::Version;
use serde::Deserialize;
use std::time::Duration;

pub const MANIFEST_URL: &str = "https://milmit.net/modemdeck/update.json";
pub const WEBSITE_URL: &str = "https://milmit.net";

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

#[derive(Debug, Clone)]
pub enum UpdateState {
    Current(UpdateManifest),
    Available(UpdateManifest),
}

pub fn check(current: &str) -> Result<UpdateState, String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .user_agent(concat!(
            "ModemDeck/",
            env!("CARGO_PKG_VERSION"),
            " (MilMit; https://milmit.net)"
        ))
        .build()
        .map_err(|error| format!("Could not initialize the update client: {error}"))?;
    let manifest = match client
        .get(MANIFEST_URL)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .and_then(|response| response.json::<UpdateManifest>())
    {
        Ok(manifest) if Version::parse(manifest.version.trim_start_matches('v')).is_ok() => {
            manifest
        }
        website_error => github_latest_release(&client, current).map_err(|error| {
            let website_error = website_error
                .err()
                .map(|error| error.to_string())
                .unwrap_or_else(|| "the MilMit manifest version is invalid".into());
            format!("Update sources are unavailable ({website_error}; GitHub fallback: {error})")
        })?,
    };
    if newer_than(&manifest.version, current) {
        Ok(UpdateState::Available(manifest))
    } else {
        Ok(UpdateState::Current(manifest))
    }
}

fn github_latest_release(
    client: &reqwest::blocking::Client,
    current: &str,
) -> Result<UpdateManifest, String> {
    let current_version = Version::parse(current.trim_start_matches('v'))
        .map_err(|error| format!("Current application version is invalid: {error}"))?;
    let releases: Vec<GithubRelease> = client
        .get("https://api.github.com/repos/MilMit/modemdeck-linux/releases?per_page=100")
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .map_err(|error| error.to_string())?;

    releases
        .into_iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = Version::parse(release.tag_name.trim_start_matches('v')).ok()?;
            if !release_allowed(&current_version, &version, release.prerelease) {
                return None;
            }
            Some((version, release))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .map(|(version, release)| UpdateManifest {
            version: version.to_string(),
            url: release.html_url,
            notes: release.body,
        })
        .ok_or_else(|| "No compatible public GitHub release was found".into())
}

fn release_allowed(current: &Version, candidate: &Version, github_prerelease: bool) -> bool {
    !current.pre.is_empty() || (!github_prerelease && candidate.pre.is_empty())
}

fn newer_than(remote: &str, local: &str) -> bool {
    let parse = |value: &str| Version::parse(value.trim_start_matches('v')).ok();
    match (parse(remote), parse(local)) {
        (Some(remote), Some(local)) => {
            if local.pre.is_empty() && !remote.pre.is_empty() {
                return false;
            }
            remote > local
        }
        _ => false,
    }
}

pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    let status = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let status: std::io::Result<std::process::Child> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Unsupported OS",
    ));
    status
        .map(|_| ())
        .map_err(|e| format!("Could not open link: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{newer_than, release_allowed};
    use semver::Version;

    #[test]
    fn update_comparison_obeys_semver_prerelease_order() {
        assert!(!newer_than("2.0.0-beta.1", "1.0.0"));
        assert!(newer_than("2.0.0", "2.0.0-beta.1"));
        assert!(newer_than("2.0.0-beta.2", "2.0.0-beta.1"));
        assert!(!newer_than("not-a-version", "1.0.0"));
    }

    #[test]
    fn stable_channel_never_selects_github_prereleases() {
        let stable = Version::parse("1.0.0").unwrap();
        let beta = Version::parse("2.0.0-beta.1").unwrap();
        let next_stable = Version::parse("2.0.0").unwrap();
        assert!(!release_allowed(&stable, &beta, true));
        assert!(!release_allowed(&stable, &beta, false));
        assert!(release_allowed(&stable, &next_stable, false));
        assert!(release_allowed(&beta, &beta, true));
        assert!(release_allowed(&beta, &next_stable, false));
    }
}
