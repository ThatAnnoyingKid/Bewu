use std::path::Path;

/// Asynchronously check if a path exists, returning an error if it could not be determined.
pub async fn try_exists<P>(path: P) -> std::io::Result<bool>
where
    P: AsRef<Path>,
{
    match tokio::fs::metadata(path.as_ref()).await {
        Ok(_metadata) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}
