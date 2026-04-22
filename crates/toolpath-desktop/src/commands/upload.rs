use serde::Serialize;
use serde_json::Value;

use crate::error::{DesktopError, DesktopResult};

/// Response returned from a successful (stubbed) Pathbase upload.
///
/// The real Pathbase service does not exist yet. We validate the payload,
/// log an event to stderr so it's visible in `cargo tauri dev`, and hand
/// back a mock URL the frontend can display. When the real API is ready,
/// replace the body of [`upload_to_pathbase`] without changing the IPC
/// signature.
#[derive(Debug, Clone, Serialize)]
pub struct UploadResult {
    pub url: String,
    pub status: &'static str,
    pub stub: bool,
}

const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024;

#[tauri::command]
pub fn upload_to_pathbase(document: Value) -> DesktopResult<UploadResult> {
    let json = serde_json::to_string(&document)?;

    if json.len() > MAX_UPLOAD_BYTES {
        return Err(DesktopError::Upload(format!(
            "document is {} bytes, over the 50MB stub limit",
            json.len()
        )));
    }

    let doc = toolpath::v1::Document::from_json(&json)
        .map_err(|e| DesktopError::InvalidInput(format!("invalid document: {e}")))?;

    let (kind, id) = match &doc {
        toolpath::v1::Document::Step(s) => ("step", s.step.id.clone()),
        toolpath::v1::Document::Path(p) => ("path", p.path.id.clone()),
        toolpath::v1::Document::Graph(g) => ("graph", g.graph.id.clone()),
    };

    let token = uuid::Uuid::new_v4().to_string();
    let url = format!("https://pathbase.dev/stub/{kind}/{token}");

    eprintln!("[toolpath-desktop] (stub) would upload {kind} id={id} -> {url}");

    Ok(UploadResult {
        url,
        status: "stubbed",
        stub: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_returns_stub_url_for_valid_path() {
        let doc = serde_json::json!({
            "Path": {
                "path": { "id": "pr-1", "head": "s1" },
                "steps": [{
                    "step": { "id": "s1", "actor": "human:x", "timestamp": "2024-01-01T00:00:00Z" },
                    "change": { "file.txt": { "raw": "@@ -0,0 +1 @@\n+hi\n" } }
                }]
            }
        });
        let res = upload_to_pathbase(doc).expect("ok");
        assert!(res.stub);
        assert_eq!(res.status, "stubbed");
        assert!(res.url.starts_with("https://pathbase.dev/stub/path/"));
    }

    #[test]
    fn upload_rejects_bad_document() {
        let err = upload_to_pathbase(serde_json::json!({ "wat": 1 })).unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
    }
}
