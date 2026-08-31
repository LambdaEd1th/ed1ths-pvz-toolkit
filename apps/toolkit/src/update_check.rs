use dioxus::prelude::*;
use semver::Version;

pub(crate) const LATEST_RELEASE_URL: &str =
    "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit/releases/latest";

const FETCH_LATEST_RELEASE: &str = r#"
return (async function () {
    let timeout;
    try {
        const controller = typeof AbortController === "function"
            ? new AbortController()
            : null;
        timeout = controller
            ? setTimeout(() => controller.abort(), 10000)
            : undefined;
        const response = await fetch(
            "https://api.github.com/repos/LambdaEd1th/ed1ths-pvz-toolkit/releases/latest",
            {
                cache: "no-store",
                ...(controller ? { signal: controller.signal } : {}),
            },
        );
        if (!response.ok) {
            dioxus.send(JSON.stringify({
                ok: false,
                error: `GitHub returned HTTP ${response.status}`,
            }));
            return;
        }
        const release = await response.json();
        dioxus.send(JSON.stringify({ ok: true, tag: release.tag_name }));
    } catch (error) {
        const message = error && error.name === "AbortError"
            ? "The update request timed out"
            : String(error || "The update request failed");
        dioxus.send(JSON.stringify({ ok: false, error: message }));
    } finally {
        if (timeout !== undefined) {
            clearTimeout(timeout);
        }
    }
})();
"#;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UpdateCheckResult {
    UpToDate,
    Available { version: String },
}

pub(crate) async fn check(current_version: &str) -> Result<UpdateCheckResult, String> {
    let mut evaluator = document::eval(FETCH_LATEST_RELEASE);
    let payload = evaluator
        .recv::<String>()
        .await
        .map_err(|error| format!("update response unavailable: {error}"))?;
    parse_response(&payload, current_version)
}

fn parse_response(payload: &str, current_version: &str) -> Result<UpdateCheckResult, String> {
    let value: serde_json::Value = serde_json::from_str(payload)
        .map_err(|error| format!("invalid update response: {error}"))?;
    if value.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
        return Err(value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("update request failed")
            .to_string());
    }

    let tag = value
        .get("tag")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "latest release is missing a version tag".to_string())?;
    compare_versions(tag, current_version)
}

fn compare_versions(tag: &str, current_version: &str) -> Result<UpdateCheckResult, String> {
    let latest = Version::parse(tag.trim().trim_start_matches(['v', 'V']))
        .map_err(|error| format!("invalid latest release version: {error}"))?;
    let current = Version::parse(current_version)
        .map_err(|error| format!("invalid current application version: {error}"))?;

    if latest > current {
        Ok(UpdateCheckResult::Available {
            version: latest.to_string(),
        })
    } else {
        Ok(UpdateCheckResult::UpToDate)
    }
}

#[cfg(test)]
mod tests {
    use super::{UpdateCheckResult, compare_versions, parse_response};

    #[test]
    fn accepts_release_tags_with_a_v_prefix() {
        assert_eq!(
            compare_versions("v0.1.4", "0.1.3"),
            Ok(UpdateCheckResult::Available {
                version: "0.1.4".to_string(),
            })
        );
    }

    #[test]
    fn compares_semver_components_numerically() {
        assert_eq!(
            compare_versions("v0.1.10", "0.1.9"),
            Ok(UpdateCheckResult::Available {
                version: "0.1.10".to_string(),
            })
        );
    }

    #[test]
    fn equal_or_older_releases_are_up_to_date() {
        assert_eq!(
            compare_versions("v0.1.3", "0.1.3"),
            Ok(UpdateCheckResult::UpToDate)
        );
        assert_eq!(
            compare_versions("v0.1.2", "0.1.3"),
            Ok(UpdateCheckResult::UpToDate)
        );
    }

    #[test]
    fn parses_github_payload_and_rejects_errors() {
        assert_eq!(
            parse_response(r#"{"ok":true,"tag":"v0.2.0"}"#, "0.1.3"),
            Ok(UpdateCheckResult::Available {
                version: "0.2.0".to_string(),
            })
        );
        assert_eq!(
            parse_response(r#"{"ok":false,"error":"HTTP 403"}"#, "0.1.3"),
            Err("HTTP 403".to_string())
        );
    }

    #[test]
    fn rejects_invalid_release_tags() {
        assert!(compare_versions("latest", "0.1.3").is_err());
    }
}
