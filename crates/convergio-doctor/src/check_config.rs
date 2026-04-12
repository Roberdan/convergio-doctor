//! Config validation check — loads config.toml and validates fields.
//!
//! Split from check_advanced.rs to stay under 250 lines.

use crate::checks::CheckStatus;

/// Load config from disk and validate.
pub fn check_config_valid() -> (CheckStatus, String) {
    let path = resolve_config_path();
    let Some(path) = path else {
        return (
            CheckStatus::Warn,
            "No config file found (using defaults)".into(),
        );
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            return (
                CheckStatus::Fail,
                format!("Cannot read {}: {e}", path.display()),
            )
        }
    };

    let config: convergio_types::config::ConvergioConfig = match toml::from_str(&content) {
        Ok(c) => c,
        Err(e) => return (CheckStatus::Fail, format!("Invalid TOML: {e}")),
    };

    let issues = validate_config(&config);
    if issues.is_empty() {
        (
            CheckStatus::Pass,
            format!("Config valid: {}", path.display()),
        )
    } else {
        (
            CheckStatus::Warn,
            format!("{} issues: {}", issues.len(), issues.join("; ")),
        )
    }
}

fn resolve_config_path() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("CONVERGIO_CONFIG") {
        let path = std::path::PathBuf::from(&p);
        // Canonicalize to resolve symlinks and prevent path tricks
        if let Ok(canonical) = path.canonicalize() {
            return Some(canonical);
        }
        if path.exists() {
            return Some(path);
        }
    }
    let home = dirs::home_dir()?;
    let path = home.join(".convergio/config.toml");
    if path.exists() {
        return Some(path);
    }
    None
}

/// Inline config validation (mirrors convergio-server::config_validation).
fn validate_config(config: &convergio_types::config::ConvergioConfig) -> Vec<String> {
    let mut issues = Vec::new();
    if config.daemon.port < 1024 {
        issues.push(format!("[daemon] port {} below 1024", config.daemon.port));
    }
    if let Some(ref tz) = config.daemon.timezone {
        if !tz.contains('/') || tz.contains(' ') || tz.len() < 5 {
            issues.push(format!("[daemon] timezone '{tz}' not IANA format"));
        }
    }
    if let Some(ref qh) = config.daemon.quiet_hours {
        if !looks_like_time_range(qh) {
            issues.push(format!("[daemon] quiet_hours '{qh}' invalid"));
        }
    }
    let known_transports = ["lan", "tailscale", "manual"];
    if !known_transports.contains(&config.mesh.transport.as_str()) {
        issues.push(format!(
            "[mesh] transport '{}' unknown",
            config.mesh.transport
        ));
    }
    let known_discovery = ["mdns", "static", "tailscale"];
    if !known_discovery.contains(&config.mesh.discovery.as_str()) {
        issues.push(format!(
            "[mesh] discovery '{}' unknown",
            config.mesh.discovery
        ));
    }
    if config.kernel.max_tokens == 0 {
        issues.push("[kernel] max_tokens must be > 0".into());
    }
    issues
}

fn looks_like_time_range(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return false;
    }
    parts.iter().all(|p| {
        let hm: Vec<&str> = p.split(':').collect();
        hm.len() == 2
            && hm[0].parse::<u32>().is_ok_and(|h| h <= 23)
            && hm[1].parse::<u32>().is_ok_and(|m| m <= 59)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_range_valid() {
        assert!(looks_like_time_range("23:00-07:00"));
        assert!(looks_like_time_range("00:00-23:59"));
    }

    #[test]
    fn time_range_invalid() {
        assert!(!looks_like_time_range("midnight"));
        assert!(!looks_like_time_range("25:00-07:00"));
        assert!(!looks_like_time_range(""));
    }

    #[test]
    fn default_config_valid() {
        let cfg = convergio_types::config::ConvergioConfig::default();
        assert!(validate_config(&cfg).is_empty());
    }

    #[test]
    fn bad_port_detected() {
        let mut cfg = convergio_types::config::ConvergioConfig::default();
        cfg.daemon.port = 80;
        let issues = validate_config(&cfg);
        assert!(issues.iter().any(|i| i.contains("port")));
    }
}
