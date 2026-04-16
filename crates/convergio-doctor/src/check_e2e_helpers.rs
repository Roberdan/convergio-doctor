//! Shared HTTP client and test data utilities for E2E doctor checks.

/// Prefix for all test data created by doctor E2E checks.
pub const TEST_PREFIX: &str = "_doctor_test_";
pub const TEST_SLUG_PREFIX: &str = "doctor-test-";

/// Monotonic-ish millisecond timestamp for unique test identifiers.
pub fn ts_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

/// Generate a unique test name with the doctor prefix.
pub fn test_name(kind: &str) -> String {
    format!("{TEST_PREFIX}{kind}_{}", ts_ms())
}

pub(crate) fn matches_test_marker(value: &str) -> bool {
    value.contains(TEST_PREFIX) || value.contains(TEST_SLUG_PREFIX)
}

/// HTTP client for doctor E2E checks against the live daemon.
pub struct DoctorHttpClient {
    client: reqwest::blocking::Client,
    base_url: String,
    token: String,
}

impl Default for DoctorHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DoctorHttpClient {
    pub fn new() -> Self {
        Self::with_base("http://localhost:8420")
    }

    pub fn with_base(url: &str) -> Self {
        let sanitized = url.trim_end_matches('/').to_string();
        // SSRF mitigation: only allow http(s) scheme and reject obviously
        // crafted URLs (file://, ftp://, gopher://, etc.)
        assert!(
            sanitized.starts_with("http://") || sanitized.starts_with("https://"),
            "DoctorHttpClient: only http(s) URLs are allowed, got: {sanitized}"
        );
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("HTTP client");
        Self {
            client,
            base_url: sanitized,
            token: std::env::var("CONVERGIO_AUTH_TOKEN").unwrap_or_else(|_| "dev-local".into()),
        }
    }

    pub fn get(&self, path: &str) -> Result<(u16, serde_json::Value), String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
        Ok((status, body))
    }

    pub fn post_json(
        &self,
        path: &str,
        json: &serde_json::Value,
    ) -> Result<(u16, serde_json::Value), String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(json)
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
        Ok((status, body))
    }

    pub fn put_json(
        &self,
        path: &str,
        json: &serde_json::Value,
    ) -> Result<(u16, serde_json::Value), String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(json)
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
        Ok((status, body))
    }

    pub fn delete(&self, path: &str) -> Result<(u16, serde_json::Value), String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
        Ok((status, body))
    }

    pub fn get_no_auth(&self, path: &str) -> Result<(u16, serde_json::Value), String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self.client.get(&url).send().map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
        Ok((status, body))
    }

    pub fn get_with_token(
        &self,
        path: &str,
        token: &str,
    ) -> Result<(u16, serde_json::Value), String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {token}"))
            .send()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let body: serde_json::Value = resp.json().unwrap_or(serde_json::Value::Null);
        Ok((status, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_has_prefix() {
        let name = test_name("plan");
        assert!(name.starts_with(TEST_PREFIX));
        assert!(name.contains("plan"));
    }

    #[test]
    fn test_name_unique() {
        let a = test_name("x");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = test_name("x");
        assert_ne!(a, b);
    }

    #[test]
    fn test_marker_matches_slugified_values() {
        assert!(matches_test_marker("_doctor_test_plan_1"));
        assert!(matches_test_marker("doctor-test-org-123"));
        assert!(!matches_test_marker("convergio-io"));
    }
}
