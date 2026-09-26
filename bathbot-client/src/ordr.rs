use std::time::Duration;

use eyre::{Result, WrapErr};
use tokio::time::timeout;

use crate::{Client, site::Site};

/// The `og:video` URL o!rdr serves for a removed/rotated video.
const ORDR_REMOVED_VIDEO: &str = "removedvideo";

/// How long to wait for the watch-page probe before giving up.
///
/// The hyper client has no timeout of its own, so a slow o!rdr would otherwise
/// stall the render interaction. This is a small HTML page, not the video.
const ORDR_PROBE_TIMEOUT: Duration = Duration::from_secs(4);

impl Client {
    /// Whether the stored o!rdr watch-page `video_url` still points at a real video.
    ///
    /// o!rdr auto-deletes old replay videos. The stored `video_url` is a watch-page
    /// (`https://link.issou.best/...`); for a deleted video it still returns
    /// `200 text/html`, so a status-code check cannot detect the miss. Instead we
    /// GET the page and read its `og:video` meta - o!rdr points that at a
    /// `removedvideo.mp4` placeholder when the video is gone.
    ///
    /// Fails open: any request/parse error is treated as "alive", so a slow or
    /// unreachable o!rdr degrades to the previous behaviour (show the cached link)
    /// rather than hiding a working one.
    pub async fn is_ordr_video_alive(&self, video_url: &str) -> Result<bool> {
        let request = self.make_get_request(video_url, Site::Ordr);

        let bytes = timeout(ORDR_PROBE_TIMEOUT, request)
            .await
            .wrap_err("Timed out fetching o!rdr watch page")?
            .wrap_err("Failed to fetch o!rdr watch page")?;

        let html = String::from_utf8_lossy(&bytes).into_owned();

        let Some(og_video) = parse_og_video(&html) else {
            // No video meta at all - treat as dead.
            return Ok(false);
        };

        Ok(!og_video.contains(ORDR_REMOVED_VIDEO))
    }
}

/// Extract the URL of the `og:video` meta tag from an o!rdr watch page.
fn parse_og_video(html: &str) -> Option<String> {
    const MARKER: &str = "og:video\" content=\"";

    let start = html.find(MARKER)? + MARKER.len();
    let rest = html.get(start..)?;
    let end = rest.find('"')?;

    Some(rest.get(..end)?.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{ORDR_REMOVED_VIDEO, parse_og_video};

    const LIVE: &str = r#"<head>
    <meta property="og:video" content="https://cdn-video-1.issou.best/ordr/75s5cad7spTnHKWf5mw2puBt.mp4" />
    <meta property="og:video:url" content="https://cdn-video-1.issou.best/ordr/75s5cad7spTnHKWf5mw2puBt.mp4" />
</head>"#;

    const DEAD: &str = r#"<head>
    <meta property="og:title" content="This video got removed from o!rdr." />
    <meta property="og:video" content="https://dl.issou.best/ordr/removedvideo.mp4" />
    <meta property="og:video:url" content="https://dl.issou.best/ordr/removedvideo.mp4" />
</head>"#;

    #[test]
    fn live_video_url() {
        let url = parse_og_video(LIVE).expect("live page has og:video");
        assert!(url.contains(".mp4"));
        assert!(!url.contains(ORDR_REMOVED_VIDEO));
    }

    #[test]
    fn dead_video_url_points_at_placeholder() {
        let url = parse_og_video(DEAD).expect("dead page has og:video");
        assert!(url.contains(ORDR_REMOVED_VIDEO));
    }

    #[test]
    fn missing_meta_returns_none() {
        assert_eq!(parse_og_video("<html></html>"), None);
    }
}
