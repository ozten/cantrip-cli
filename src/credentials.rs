use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub api_key: String,
    pub daemon_url: String,
}

/// Returns the path to the credentials file: ~/.config/cantrip/credentials.json
fn credentials_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("cantrip").join("credentials.json"))
}

/// Read stored credentials. Returns None if file doesn't exist or is invalid.
/// Warns on corrupt files or insecure permissions.
pub fn load() -> Option<Credentials> {
    let path = credentials_path()?;
    if !path.exists() {
        return None;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            let mode = meta.permissions().mode() & 0o777;
            if mode != 0o600 {
                eprintln!(
                    "Warning: credential file has insecure permissions ({:o}). \
                     Run: chmod 600 {}",
                    mode,
                    path.display()
                );
            }
        }
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: cannot read {}: {}", path.display(), e);
            return None;
        }
    };

    match serde_json::from_str::<Credentials>(&contents) {
        Ok(creds) => Some(creds),
        Err(_) => {
            eprintln!(
                "Warning: credential file is corrupted. Run `cantrip login` to re-authenticate."
            );
            None
        }
    }
}

/// Save credentials to disk with 0600 permissions.
pub fn save(creds: &Credentials) -> Result<(), String> {
    let path = credentials_path().ok_or("cannot determine config directory")?;
    let dir = path.parent().unwrap();

    std::fs::create_dir_all(dir)
        .map_err(|e| format!("cannot create {}: {}", dir.display(), e))?;

    let json = serde_json::to_string_pretty(creds)
        .map_err(|e| format!("cannot serialize credentials: {e}"))?;

    std::fs::write(&path, &json)
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("cannot set permissions on {}: {}", path.display(), e))?;
    }

    Ok(())
}

/// Delete the credentials file. Returns true if file existed.
pub fn delete() -> bool {
    let Some(path) = credentials_path() else {
        return false;
    };
    if !path.exists() {
        return false;
    }
    if let Err(e) = std::fs::remove_file(&path) {
        eprintln!("Warning: cannot remove {}: {}", path.display(), e);
        return false;
    }
    true
}
