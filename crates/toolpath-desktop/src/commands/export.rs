use serde_json::Value;
use std::path::PathBuf;

use crate::error::{DesktopError, DesktopResult};

/// Write a Toolpath document to disk as pretty-printed JSON.
///
/// The frontend is responsible for prompting the user with a save dialog
/// (via `@tauri-apps/plugin-dialog`) and passing us the chosen absolute path
/// plus the derived document as a JSON value. We parse it back through the
/// `Document` type so bad payloads are rejected before touching the
/// filesystem.
#[tauri::command]
pub fn save_document(document: Value, out_path: String) -> DesktopResult<String> {
    if out_path.is_empty() {
        return Err(DesktopError::InvalidInput("out_path is empty".into()));
    }

    // Round-trip through Document to enforce schema-shape at the boundary.
    let json = serde_json::to_string(&document)?;
    let doc = toolpath::v1::Document::from_json(&json)
        .map_err(|e| DesktopError::InvalidInput(format!("invalid document: {e}")))?;
    let pretty = doc
        .to_json_pretty()
        .map_err(|e| DesktopError::Io(format!("serialize: {e}")))?;

    let path = PathBuf::from(&out_path);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, pretty)?;

    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_document() -> Value {
        serde_json::json!({
            "Path": {
                "path": {
                    "id": "p1",
                    "head": "s1"
                },
                "steps": [{
                    "step": {
                        "id": "s1",
                        "actor": "human:tester",
                        "timestamp": "2024-01-01T00:00:00Z"
                    },
                    "change": {
                        "file.txt": { "raw": "@@ -0,0 +1 @@\n+hi\n" }
                    }
                }]
            }
        })
    }

    #[test]
    fn save_document_writes_pretty_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.path.json");
        let written =
            save_document(minimal_document(), path.to_string_lossy().into_owned()).expect("save");
        let body = std::fs::read_to_string(&written).unwrap();
        // Pretty output uses indentation.
        assert!(body.contains("  \"path\""));
        // Round-trips through Document.
        let doc = toolpath::v1::Document::from_json(&body).expect("parse");
        match doc {
            toolpath::v1::Document::Path(p) => {
                assert_eq!(p.path.id, "p1");
                assert_eq!(p.path.head, "s1");
            }
            _ => panic!("expected Path"),
        }
    }

    #[test]
    fn save_document_rejects_empty_path() {
        let err = save_document(minimal_document(), String::new()).unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
    }

    #[test]
    fn save_document_rejects_invalid_payload() {
        let err = save_document(
            serde_json::json!({ "nonsense": true }),
            "/tmp/should-not-be-written".into(),
        )
        .unwrap_err();
        assert!(matches!(err, DesktopError::InvalidInput(_)));
    }
}
