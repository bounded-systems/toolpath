use anyhow::Result;
use std::path::PathBuf;
use toolpath::v1::Graph;

use crate::io::read_document_auto;

pub fn run(input: PathBuf) -> Result<()> {
    match read_document_auto(&input) {
        Ok(doc) => {
            println!("Valid: {}", describe(&doc));
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("Invalid: {}", e)),
    }
}

fn describe(doc: &Graph) -> String {
    let path_count = doc.paths.len();
    format!(
        "Graph (id: {}, {} {})",
        doc.graph.id,
        path_count,
        if path_count == 1 { "path" } else { "paths" }
    )
}

#[cfg(test)]
fn validate_content(content: &str) -> Result<()> {
    match Graph::from_json(content) {
        Ok(doc) => {
            println!("Valid: {}", describe(&doc));
            Ok(())
        }
        Err(e) => Err(anyhow::anyhow!("Invalid: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_validate_empty_graph() {
        let json = r#"{"graph":{"id":"g1"},"paths":[]}"#;
        assert!(validate_content(json).is_ok());
    }

    #[test]
    fn test_validate_single_path_graph() {
        let json = r#"{"graph":{"id":"g1"},"paths":[{"path":{"id":"p1","head":"s1"},"steps":[{"step":{"id":"s1","actor":"human:alex","timestamp":"2026-01-01T00:00:00Z"},"change":{}}]}]}"#;
        assert!(validate_content(json).is_ok());
    }

    #[test]
    fn test_validate_invalid_json() {
        assert!(validate_content("not json").is_err());
    }

    #[test]
    fn test_validate_missing_required_field() {
        assert!(validate_content(r#"{"paths":[]}"#).is_err());
    }

    #[test]
    fn test_run_with_temp_file() {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        write!(f, r#"{{"graph":{{"id":"g1"}},"paths":[]}}"#).unwrap();
        f.flush().unwrap();
        assert!(run(f.path().to_path_buf()).is_ok());
    }

    #[test]
    fn test_run_nonexistent_file() {
        assert!(run(PathBuf::from("/nonexistent/file.json")).is_err());
    }
}
