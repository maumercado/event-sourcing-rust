//! Application configuration loaded from environment variables.

/// Server configuration with sensible defaults.
///
/// Reads from environment variables:
/// - `HOST` — bind address (default: `"0.0.0.0"`)
/// - `PORT` — listen port (default: `3000`)
/// - `RUST_LOG` — tracing filter directive (default: `"info"`)
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

impl Config {
    /// Loads configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            log_level: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        }
    }

    /// Returns the `"host:port"` bind address string.
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
            log_level: "info".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let config = Config::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3000);
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_addr_formatting() {
        let config = Config {
            host: "127.0.0.1".to_string(),
            port: 8080,
            log_level: "debug".to_string(),
        };
        assert_eq!(config.addr(), "127.0.0.1:8080");
    }

    #[test]
    fn test_addr_default() {
        let config = Config::default();
        assert_eq!(config.addr(), "0.0.0.0:3000");
    }
}
