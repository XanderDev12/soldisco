use std::{
    env,
    net::{IpAddr, SocketAddr},
};

use axum::http::HeaderValue;
use thiserror::Error;

const DEFAULT_APP_ENV: &str = "development";
const DEFAULT_LOG_LEVEL: &str = "info,soldisco_server=debug";
const DEFAULT_API_HOST: &str = "127.0.0.1";
const DEFAULT_API_PORT: &str = "8080";
const DEFAULT_WEB_ORIGIN: &str = "http://localhost:3000";
const DEFAULT_DATABASE_MAX_CONNECTIONS: &str = "5";

pub struct Config {
    pub app_env: String,
    pub log_level: String,
    pub api_address: SocketAddr,
    pub web_origin: String,
    pub database_url: String,
    pub database_max_connections: u32,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ConfigError {
    #[error("required environment variable {0} is missing")]
    Missing(&'static str),
    #[error("{name} has an invalid value: {reason}")]
    Invalid {
        name: &'static str,
        reason: &'static str,
    },
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        Self::from_values(|name| env::var(name).ok())
    }

    fn from_values(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let app_env = value_or(&get, "APP_ENV", DEFAULT_APP_ENV);
        let log_level = value_or(&get, "LOG_LEVEL", DEFAULT_LOG_LEVEL);
        let api_host = value_or(&get, "API_HOST", DEFAULT_API_HOST);
        let api_port = value_or(&get, "API_PORT", DEFAULT_API_PORT);
        let web_origin = value_or(&get, "WEB_ORIGIN", DEFAULT_WEB_ORIGIN);
        let database_url = get("DATABASE_URL")
            .filter(|value| !value.trim().is_empty())
            .ok_or(ConfigError::Missing("DATABASE_URL"))?;
        let database_max_connections = value_or(
            &get,
            "DATABASE_MAX_CONNECTIONS",
            DEFAULT_DATABASE_MAX_CONNECTIONS,
        );

        let host: IpAddr = api_host.parse().map_err(|_| ConfigError::Invalid {
            name: "API_HOST",
            reason: "expected an IP address",
        })?;
        if !host.is_loopback() {
            return Err(ConfigError::Invalid {
                name: "API_HOST",
                reason: "the local milestone requires a loopback address",
            });
        }

        let port: u16 = api_port.parse().map_err(|_| ConfigError::Invalid {
            name: "API_PORT",
            reason: "expected a port from 1 to 65535",
        })?;
        if port == 0 {
            return Err(ConfigError::Invalid {
                name: "API_PORT",
                reason: "port zero is not allowed",
            });
        }

        HeaderValue::from_str(&web_origin).map_err(|_| ConfigError::Invalid {
            name: "WEB_ORIGIN",
            reason: "expected one valid HTTP origin",
        })?;
        if !(web_origin.starts_with("http://") || web_origin.starts_with("https://")) {
            return Err(ConfigError::Invalid {
                name: "WEB_ORIGIN",
                reason: "expected an http:// or https:// origin",
            });
        }

        if !(database_url.starts_with("postgres://") || database_url.starts_with("postgresql://")) {
            return Err(ConfigError::Invalid {
                name: "DATABASE_URL",
                reason: "expected a PostgreSQL connection URL",
            });
        }

        let database_max_connections =
            database_max_connections
                .parse::<u32>()
                .map_err(|_| ConfigError::Invalid {
                    name: "DATABASE_MAX_CONNECTIONS",
                    reason: "expected a positive integer",
                })?;
        if database_max_connections == 0 {
            return Err(ConfigError::Invalid {
                name: "DATABASE_MAX_CONNECTIONS",
                reason: "must be at least one",
            });
        }

        Ok(Self {
            app_env,
            log_level,
            api_address: SocketAddr::new(host, port),
            web_origin,
            database_url,
            database_max_connections,
        })
    }
}

fn value_or(get: &impl Fn(&str) -> Option<String>, name: &str, default: &str) -> String {
    get(name)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Config, ConfigError};

    fn base_values() -> BTreeMap<String, String> {
        BTreeMap::from([(
            "DATABASE_URL".to_owned(),
            "postgres://soldisco:local@127.0.0.1/soldisco".to_owned(),
        )])
    }

    #[test]
    fn defaults_are_local_only() {
        let values = base_values();
        let config = Config::from_values(|name| values.get(name).cloned())
            .expect("local defaults should be valid");

        assert!(config.api_address.ip().is_loopback());
        assert_eq!(config.api_address.port(), 8080);
        assert_eq!(config.web_origin, "http://localhost:3000");
    }

    #[test]
    fn database_url_is_required_without_logging_its_value() {
        let values: BTreeMap<String, String> = BTreeMap::new();
        let error = match Config::from_values(|name| values.get(name).cloned()) {
            Ok(_) => panic!("missing database URL must fail"),
            Err(error) => error,
        };

        assert_eq!(error, ConfigError::Missing("DATABASE_URL"));
    }

    #[test]
    fn public_bind_address_is_rejected_for_the_local_milestone() {
        let mut values = base_values();
        values.insert("API_HOST".to_owned(), "0.0.0.0".to_owned());

        let error = match Config::from_values(|name| values.get(name).cloned()) {
            Ok(_) => panic!("public bind must fail closed"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "API_HOST",
                ..
            }
        ));
    }
}
