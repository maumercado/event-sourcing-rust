//! Application configuration loaded from environment variables.

/// Server configuration with sensible defaults.
///
/// Reads from environment variables:
/// - `HOST` — bind address (default: `"0.0.0.0"`)
/// - `PORT` — listen port (default: `3000`)
/// - `RUST_LOG` — tracing filter directive (default: `"info"`)
/// - `DATABASE_URL` — PostgreSQL connection string (default: `None`, uses in-memory store)
/// - `DB_MAX_CONNECTIONS` — max database pool connections (default: `10`)
#[derive(Debug, Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub database_url: Option<String>,
    pub db_max_connections: u32,
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
            database_url: std::env::var("DATABASE_URL").ok(),
            db_max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
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
            database_url: None,
            db_max_connections: 10,
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
            database_url: None,
            db_max_connections: 10,
        };
        assert_eq!(config.addr(), "127.0.0.1:8080");
    }

    #[test]
    fn test_addr_default() {
        let config = Config::default();
        assert_eq!(config.addr(), "0.0.0.0:3000");
    }

    #[test]
    fn test_default_database_fields() {
        let config = Config::default();
        assert!(config.database_url.is_none());
        assert_eq!(config.db_max_connections, 10);
    }
}
