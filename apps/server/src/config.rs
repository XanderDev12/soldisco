use std::{
    env,
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use axum::http::{HeaderValue, Uri};
use soldisco_api_contracts::PrefilterDefaults;
use soldisco_domain::{Commitment, Network};
use thiserror::Error;

const DEFAULT_APP_ENV: &str = "development";
const DEFAULT_LOG_LEVEL: &str = "info,soldisco_server=debug";
const DEFAULT_API_HOST: &str = "127.0.0.1";
const DEFAULT_API_PORT: &str = "8080";
const DEFAULT_WEB_ORIGIN: &str = "http://localhost:3000";
const DEFAULT_DATABASE_MAX_CONNECTIONS: &str = "5";
const DEFAULT_DATABASE_MAX_BYTES: &str = "5368709120";
const DEFAULT_DISCOVERY_SNAPSHOT_LIMIT: &str = "500";
const DEFAULT_RETENTION_TERMINAL_HISTORY_HOURS: &str = "24";
const DEFAULT_RETENTION_PROJECTION_EVENTS_HOURS: &str = "24";
const DEFAULT_RETENTION_QUARANTINE_HOURS: &str = "168";
const DEFAULT_RETENTION_INTERVAL_MS: &str = "60000";
const DEFAULT_RETENTION_BATCH_SIZE: &str = "5000";
const DEFAULT_SOLANA_NETWORK: &str = "mainnet";
const DEFAULT_SOLANA_RPC_HTTP_URL: &str = "https://api.mainnet-beta.solana.com";
const DEFAULT_SOLANA_RPC_WS_URL: &str = "wss://api.mainnet-beta.solana.com";
const DEFAULT_SOLANA_COMMITMENT: &str = "confirmed";
const DEFAULT_SOLANA_REQUEST_TIMEOUT_MS: &str = "5000";
const DEFAULT_SOLANA_RECONNECT_DELAY_MS: &str = "1000";
const DEFAULT_SOLANA_RPC_MAX_IN_FLIGHT: &str = "4";
const DEFAULT_SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND: &str = "1";
const DEFAULT_SOLANA_DISCOVERY_RPC_RATE_LIMIT_COOLDOWN_MS: &str = "5000";
const DEFAULT_SOLANA_SUBSCRIPTION_IDLE_TIMEOUT_MS: &str = "30000";
const DEFAULT_DISCOVERY_MAX_EVENT_AGE_MS: &str = "15000";
const DEFAULT_DISCOVERY_OBSERVATION_WINDOW_MS: &str = "60000";
const DEFAULT_DISCOVERY_MAX_ACTIVE_WINDOWS: &str = "128";
const DEFAULT_COLLECTOR_QUEUE_CAPACITY: &str = "2048";
const DEFAULT_STREAM_START_TIMEOUT_MS: &str = "20000";

#[derive(Clone)]
pub struct Config {
    pub app_env: String,
    pub log_level: String,
    pub api_address: SocketAddr,
    pub web_origin: String,
    pub database_url: String,
    pub database_max_connections: u32,
    pub database_max_bytes: u64,
    pub discovery_snapshot_limit: u32,
    pub retention_terminal_history: Duration,
    pub retention_projection_events: Duration,
    pub retention_quarantine: Duration,
    pub retention_interval: Duration,
    pub retention_batch_size: u32,
    pub solana_network: Network,
    pub solana_rpc_http_url: String,
    pub solana_rpc_ws_url: String,
    pub solana_commitment: Commitment,
    pub solana_request_timeout: Duration,
    pub solana_reconnect_delay: Duration,
    pub solana_rpc_max_in_flight: usize,
    pub solana_discovery_rpc_requests_per_second: u32,
    pub solana_discovery_rpc_rate_limit_cooldown: Duration,
    pub solana_subscription_idle_timeout: Duration,
    pub discovery_max_event_age: Duration,
    pub discovery_observation_window: Duration,
    pub discovery_max_active_windows: usize,
    pub collector_queue_capacity: usize,
    pub stream_start_timeout: Duration,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Config")
            .field("app_env", &self.app_env)
            .field("log_level", &self.log_level)
            .field("api_address", &self.api_address)
            .field("web_origin", &self.web_origin)
            .field("database_url", &"<redacted>")
            .field("database_max_connections", &self.database_max_connections)
            .field("database_max_bytes", &self.database_max_bytes)
            .field("discovery_snapshot_limit", &self.discovery_snapshot_limit)
            .field(
                "retention_terminal_history",
                &self.retention_terminal_history,
            )
            .field(
                "retention_projection_events",
                &self.retention_projection_events,
            )
            .field("retention_quarantine", &self.retention_quarantine)
            .field("retention_interval", &self.retention_interval)
            .field("retention_batch_size", &self.retention_batch_size)
            .field("solana_network", &self.solana_network)
            .field("solana_rpc_http_url", &"<redacted>")
            .field("solana_rpc_ws_url", &"<redacted>")
            .field("solana_commitment", &self.solana_commitment)
            .field("solana_request_timeout", &self.solana_request_timeout)
            .field("solana_reconnect_delay", &self.solana_reconnect_delay)
            .field("solana_rpc_max_in_flight", &self.solana_rpc_max_in_flight)
            .field(
                "solana_discovery_rpc_requests_per_second",
                &self.solana_discovery_rpc_requests_per_second,
            )
            .field(
                "solana_discovery_rpc_rate_limit_cooldown",
                &self.solana_discovery_rpc_rate_limit_cooldown,
            )
            .field(
                "solana_subscription_idle_timeout",
                &self.solana_subscription_idle_timeout,
            )
            .field("discovery_max_event_age", &self.discovery_max_event_age)
            .field(
                "discovery_observation_window",
                &self.discovery_observation_window,
            )
            .field(
                "discovery_max_active_windows",
                &self.discovery_max_active_windows,
            )
            .field("collector_queue_capacity", &self.collector_queue_capacity)
            .field("stream_start_timeout", &self.stream_start_timeout)
            .finish()
    }
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

    #[must_use]
    pub fn prefilter_defaults(&self) -> PrefilterDefaults {
        PrefilterDefaults {
            max_event_age_ms: duration_millis(self.discovery_max_event_age),
            observation_window_ms: duration_millis(self.discovery_observation_window),
            max_active_windows: u32::try_from(self.discovery_max_active_windows)
                .expect("validated maximum active windows fit u32"),
            rpc_requests_per_second: self.solana_discovery_rpc_requests_per_second,
            rpc_max_in_flight: u32::try_from(self.solana_rpc_max_in_flight)
                .expect("validated RPC concurrency fits u32"),
            rpc_request_timeout_ms: duration_millis(self.solana_request_timeout),
            rpc_rate_limit_cooldown_ms: duration_millis(
                self.solana_discovery_rpc_rate_limit_cooldown,
            ),
        }
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
        let database_max_bytes = value_or(&get, "DATABASE_MAX_BYTES", DEFAULT_DATABASE_MAX_BYTES);
        let discovery_snapshot_limit = value_or(
            &get,
            "DISCOVERY_SNAPSHOT_LIMIT",
            DEFAULT_DISCOVERY_SNAPSHOT_LIMIT,
        );
        let retention_terminal_history_hours = value_or(
            &get,
            "RETENTION_TERMINAL_HISTORY_HOURS",
            DEFAULT_RETENTION_TERMINAL_HISTORY_HOURS,
        );
        let retention_projection_events_hours = value_or(
            &get,
            "RETENTION_PROJECTION_EVENTS_HOURS",
            DEFAULT_RETENTION_PROJECTION_EVENTS_HOURS,
        );
        let retention_quarantine_hours = value_or(
            &get,
            "RETENTION_QUARANTINE_HOURS",
            DEFAULT_RETENTION_QUARANTINE_HOURS,
        );
        let retention_interval_ms =
            value_or(&get, "RETENTION_INTERVAL_MS", DEFAULT_RETENTION_INTERVAL_MS);
        let retention_batch_size =
            value_or(&get, "RETENTION_BATCH_SIZE", DEFAULT_RETENTION_BATCH_SIZE);
        let solana_network = value_or(&get, "SOLANA_NETWORK", DEFAULT_SOLANA_NETWORK);
        let solana_rpc_http_url =
            value_or(&get, "SOLANA_RPC_HTTP_URL", DEFAULT_SOLANA_RPC_HTTP_URL);
        let solana_rpc_ws_url = value_or(&get, "SOLANA_RPC_WS_URL", DEFAULT_SOLANA_RPC_WS_URL);
        let solana_commitment = value_or(&get, "SOLANA_COMMITMENT", DEFAULT_SOLANA_COMMITMENT);
        let solana_request_timeout_ms = value_or(
            &get,
            "SOLANA_REQUEST_TIMEOUT_MS",
            DEFAULT_SOLANA_REQUEST_TIMEOUT_MS,
        );
        let solana_reconnect_delay_ms = value_or(
            &get,
            "SOLANA_RECONNECT_DELAY_MS",
            DEFAULT_SOLANA_RECONNECT_DELAY_MS,
        );
        let solana_rpc_max_in_flight = value_or(
            &get,
            "SOLANA_RPC_MAX_IN_FLIGHT",
            DEFAULT_SOLANA_RPC_MAX_IN_FLIGHT,
        );
        let solana_discovery_rpc_requests_per_second = value_or(
            &get,
            "SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND",
            DEFAULT_SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND,
        );
        let solana_discovery_rpc_rate_limit_cooldown_ms = value_or(
            &get,
            "SOLANA_DISCOVERY_RPC_RATE_LIMIT_COOLDOWN_MS",
            DEFAULT_SOLANA_DISCOVERY_RPC_RATE_LIMIT_COOLDOWN_MS,
        );
        let solana_subscription_idle_timeout_ms = value_or(
            &get,
            "SOLANA_SUBSCRIPTION_IDLE_TIMEOUT_MS",
            DEFAULT_SOLANA_SUBSCRIPTION_IDLE_TIMEOUT_MS,
        );
        let discovery_max_event_age_ms = value_or(
            &get,
            "DISCOVERY_MAX_EVENT_AGE_MS",
            DEFAULT_DISCOVERY_MAX_EVENT_AGE_MS,
        );
        let discovery_observation_window_ms = value_or(
            &get,
            "DISCOVERY_OBSERVATION_WINDOW_MS",
            DEFAULT_DISCOVERY_OBSERVATION_WINDOW_MS,
        );
        let discovery_max_active_windows = value_or(
            &get,
            "DISCOVERY_MAX_ACTIVE_WINDOWS",
            DEFAULT_DISCOVERY_MAX_ACTIVE_WINDOWS,
        );
        let collector_queue_capacity = value_or(
            &get,
            "COLLECTOR_QUEUE_CAPACITY",
            DEFAULT_COLLECTOR_QUEUE_CAPACITY,
        );
        let stream_start_timeout_ms = value_or(
            &get,
            "STREAM_START_TIMEOUT_MS",
            DEFAULT_STREAM_START_TIMEOUT_MS,
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
        let web_origin_uri = web_origin
            .parse::<Uri>()
            .map_err(|_| ConfigError::Invalid {
                name: "WEB_ORIGIN",
                reason: "expected one valid HTTP origin",
            })?;
        let valid_web_scheme = matches!(web_origin_uri.scheme_str(), Some("http") | Some("https"));
        if !valid_web_scheme
            || web_origin_uri.authority().is_none()
            || web_origin_uri
                .authority()
                .is_some_and(|authority| authority.as_str().contains('@'))
            || web_origin_uri.path() != "/"
            || web_origin_uri.query().is_some()
            || web_origin.ends_with('/')
        {
            return Err(ConfigError::Invalid {
                name: "WEB_ORIGIN",
                reason: "expected an http:// or https:// origin without credentials, path, query, or fragment",
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
        let database_max_bytes = parse_bounded_u64(
            "DATABASE_MAX_BYTES",
            &database_max_bytes,
            100 * 1024 * 1024,
            1024 * 1024 * 1024 * 1024,
        )?;
        let discovery_snapshot_limit = parse_bounded_u64(
            "DISCOVERY_SNAPSHOT_LIMIT",
            &discovery_snapshot_limit,
            1,
            5_000,
        )
        .and_then(|value| {
            u32::try_from(value).map_err(|_| ConfigError::Invalid {
                name: "DISCOVERY_SNAPSHOT_LIMIT",
                reason: "value is outside the supported range",
            })
        })?;
        let retention_terminal_history = parse_positive_hours(
            "RETENTION_TERMINAL_HISTORY_HOURS",
            &retention_terminal_history_hours,
        )?;
        let retention_projection_events = parse_positive_hours(
            "RETENTION_PROJECTION_EVENTS_HOURS",
            &retention_projection_events_hours,
        )?;
        let retention_quarantine =
            parse_positive_hours("RETENTION_QUARANTINE_HOURS", &retention_quarantine_hours)?;
        let retention_interval = parse_bounded_millisecond_duration(
            "RETENTION_INTERVAL_MS",
            &retention_interval_ms,
            1_000,
            3_600_000,
        )?;
        let retention_batch_size =
            parse_bounded_u64("RETENTION_BATCH_SIZE", &retention_batch_size, 1, 10_000).and_then(
                |value| {
                    u32::try_from(value).map_err(|_| ConfigError::Invalid {
                        name: "RETENTION_BATCH_SIZE",
                        reason: "value is outside the supported range",
                    })
                },
            )?;

        let solana_network = match solana_network.trim().to_ascii_lowercase().as_str() {
            "mainnet" | "mainnet-beta" => Network::SolanaMainnet,
            "devnet" => Network::SolanaDevnet,
            _ => {
                return Err(ConfigError::Invalid {
                    name: "SOLANA_NETWORK",
                    reason: "expected mainnet or devnet",
                });
            }
        };
        validate_url(
            "SOLANA_RPC_HTTP_URL",
            &solana_rpc_http_url,
            &["http://", "https://"],
        )?;
        validate_url(
            "SOLANA_RPC_WS_URL",
            &solana_rpc_ws_url,
            &["ws://", "wss://"],
        )?;
        let solana_commitment = match solana_commitment.trim().to_ascii_lowercase().as_str() {
            "confirmed" => Commitment::Confirmed,
            "finalized" => Commitment::Finalized,
            _ => {
                return Err(ConfigError::Invalid {
                    name: "SOLANA_COMMITMENT",
                    reason: "expected confirmed or finalized",
                });
            }
        };
        let solana_request_timeout = parse_bounded_millisecond_duration(
            "SOLANA_REQUEST_TIMEOUT_MS",
            &solana_request_timeout_ms,
            1,
            300_000,
        )?;
        let solana_reconnect_delay =
            parse_positive_duration("SOLANA_RECONNECT_DELAY_MS", &solana_reconnect_delay_ms)?;
        let solana_rpc_max_in_flight = parse_bounded_usize(
            "SOLANA_RPC_MAX_IN_FLIGHT",
            &solana_rpc_max_in_flight,
            1,
            128,
        )?;
        let solana_discovery_rpc_requests_per_second = parse_bounded_u64(
            "SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND",
            &solana_discovery_rpc_requests_per_second,
            1,
            1_000,
        )
        .and_then(|value| {
            u32::try_from(value).map_err(|_| ConfigError::Invalid {
                name: "SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND",
                reason: "value is outside the supported range",
            })
        })?;
        let solana_discovery_rpc_rate_limit_cooldown = parse_bounded_millisecond_duration(
            "SOLANA_DISCOVERY_RPC_RATE_LIMIT_COOLDOWN_MS",
            &solana_discovery_rpc_rate_limit_cooldown_ms,
            100,
            300_000,
        )?;
        let solana_subscription_idle_timeout = parse_positive_duration(
            "SOLANA_SUBSCRIPTION_IDLE_TIMEOUT_MS",
            &solana_subscription_idle_timeout_ms,
        )?;
        let discovery_max_event_age = parse_bounded_millisecond_duration(
            "DISCOVERY_MAX_EVENT_AGE_MS",
            &discovery_max_event_age_ms,
            1_000,
            300_000,
        )?;
        let discovery_observation_window = parse_bounded_millisecond_duration(
            "DISCOVERY_OBSERVATION_WINDOW_MS",
            &discovery_observation_window_ms,
            1_000,
            3_600_000,
        )?;
        if solana_request_timeout > discovery_max_event_age {
            return Err(ConfigError::Invalid {
                name: "SOLANA_REQUEST_TIMEOUT_MS",
                reason: "cannot exceed DISCOVERY_MAX_EVENT_AGE_MS",
            });
        }
        if discovery_observation_window < solana_request_timeout {
            return Err(ConfigError::Invalid {
                name: "DISCOVERY_OBSERVATION_WINDOW_MS",
                reason: "cannot be shorter than SOLANA_REQUEST_TIMEOUT_MS",
            });
        }
        let discovery_max_active_windows = parse_bounded_usize(
            "DISCOVERY_MAX_ACTIVE_WINDOWS",
            &discovery_max_active_windows,
            1,
            100_000,
        )?;
        let collector_queue_capacity = parse_bounded_usize(
            "COLLECTOR_QUEUE_CAPACITY",
            &collector_queue_capacity,
            1,
            100_000,
        )?;
        let stream_start_timeout =
            parse_positive_duration("STREAM_START_TIMEOUT_MS", &stream_start_timeout_ms)?;

        Ok(Self {
            app_env,
            log_level,
            api_address: SocketAddr::new(host, port),
            web_origin,
            database_url,
            database_max_connections,
            database_max_bytes,
            discovery_snapshot_limit,
            retention_terminal_history,
            retention_projection_events,
            retention_quarantine,
            retention_interval,
            retention_batch_size,
            solana_network,
            solana_rpc_http_url,
            solana_rpc_ws_url,
            solana_commitment,
            solana_request_timeout,
            solana_reconnect_delay,
            solana_rpc_max_in_flight,
            solana_discovery_rpc_requests_per_second,
            solana_discovery_rpc_rate_limit_cooldown,
            solana_subscription_idle_timeout,
            discovery_max_event_age,
            discovery_observation_window,
            discovery_max_active_windows,
            collector_queue_capacity,
            stream_start_timeout,
        })
    }
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).expect("validated duration milliseconds fit u64")
}

fn value_or(get: &impl Fn(&str) -> Option<String>, name: &str, default: &str) -> String {
    get(name)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn validate_url(
    name: &'static str,
    value: &str,
    allowed_schemes: &[&str],
) -> Result<(), ConfigError> {
    if value.trim() != value
        || value.chars().any(char::is_whitespace)
        || !allowed_schemes
            .iter()
            .any(|scheme| value.starts_with(scheme))
    {
        return Err(ConfigError::Invalid {
            name,
            reason: "expected an absolute URL with the required scheme",
        });
    }
    Ok(())
}

fn parse_positive_duration(name: &'static str, value: &str) -> Result<Duration, ConfigError> {
    let milliseconds = value.parse::<u64>().map_err(|_| ConfigError::Invalid {
        name,
        reason: "expected a positive integer number of milliseconds",
    })?;
    if milliseconds == 0 {
        return Err(ConfigError::Invalid {
            name,
            reason: "must be at least one millisecond",
        });
    }
    Ok(Duration::from_millis(milliseconds))
}

fn parse_positive_hours(name: &'static str, value: &str) -> Result<Duration, ConfigError> {
    let hours = parse_bounded_u64(name, value, 1, 8_760)?;
    Ok(Duration::from_secs(hours.saturating_mul(60 * 60)))
}

fn parse_bounded_millisecond_duration(
    name: &'static str,
    value: &str,
    minimum: u64,
    maximum: u64,
) -> Result<Duration, ConfigError> {
    parse_bounded_u64(name, value, minimum, maximum).map(Duration::from_millis)
}

fn parse_bounded_u64(
    name: &'static str,
    value: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, ConfigError> {
    let value = value.parse::<u64>().map_err(|_| ConfigError::Invalid {
        name,
        reason: "expected a positive integer",
    })?;
    if !(minimum..=maximum).contains(&value) {
        return Err(ConfigError::Invalid {
            name,
            reason: "value is outside the supported range",
        });
    }
    Ok(value)
}

fn parse_bounded_usize(
    name: &'static str,
    value: &str,
    minimum: usize,
    maximum: usize,
) -> Result<usize, ConfigError> {
    let value = value.parse::<usize>().map_err(|_| ConfigError::Invalid {
        name,
        reason: "expected a positive integer",
    })?;
    if !(minimum..=maximum).contains(&value) {
        return Err(ConfigError::Invalid {
            name,
            reason: "value is outside the supported range",
        });
    }
    Ok(value)
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
        assert_eq!(
            config.solana_network,
            soldisco_domain::Network::SolanaMainnet
        );
        assert_eq!(
            config.solana_commitment,
            soldisco_domain::Commitment::Confirmed
        );
        assert_eq!(config.collector_queue_capacity, 2048);
        assert_eq!(config.database_max_bytes, 5 * 1024 * 1024 * 1024);
        assert_eq!(config.discovery_snapshot_limit, 500);
        assert_eq!(
            config.retention_terminal_history,
            std::time::Duration::from_secs(24 * 60 * 60)
        );
        assert_eq!(config.retention_batch_size, 5_000);
        assert_eq!(config.solana_rpc_max_in_flight, 4);
        assert_eq!(config.solana_discovery_rpc_requests_per_second, 1);
        assert_eq!(
            config.solana_discovery_rpc_rate_limit_cooldown,
            std::time::Duration::from_secs(5)
        );
        assert_eq!(
            config.solana_subscription_idle_timeout,
            std::time::Duration::from_secs(30)
        );
        assert_eq!(
            config.discovery_max_event_age,
            std::time::Duration::from_secs(15)
        );
        assert_eq!(
            config.discovery_observation_window,
            std::time::Duration::from_secs(60)
        );
        assert_eq!(config.discovery_max_active_windows, 128);
        assert_eq!(
            config.prefilter_defaults(),
            soldisco_api_contracts::PrefilterDefaults {
                max_event_age_ms: 15_000,
                observation_window_ms: 60_000,
                max_active_windows: 128,
                rpc_requests_per_second: 1,
                rpc_max_in_flight: 4,
                rpc_request_timeout_ms: 5_000,
                rpc_rate_limit_cooldown_ms: 5_000,
            }
        );
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
    fn debug_output_redacts_database_and_rpc_credentials() {
        let mut values = base_values();
        values.insert(
            "DATABASE_URL".to_owned(),
            "postgres://soldisco:database-secret@127.0.0.1/soldisco".to_owned(),
        );
        values.insert(
            "SOLANA_RPC_HTTP_URL".to_owned(),
            "https://rpc.example.invalid/http-secret".to_owned(),
        );
        values.insert(
            "SOLANA_RPC_WS_URL".to_owned(),
            "wss://rpc.example.invalid/ws-secret".to_owned(),
        );
        let config =
            Config::from_values(|name| values.get(name).cloned()).expect("valid configuration");
        let debug = format!("{config:?}");

        assert!(!debug.contains("database-secret"));
        assert!(!debug.contains("http-secret"));
        assert!(!debug.contains("ws-secret"));
        assert!(debug.contains("<redacted>"));
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

    #[test]
    fn rpc_urls_and_commitment_are_validated() {
        let mut values = base_values();
        values.insert(
            "SOLANA_RPC_WS_URL".to_owned(),
            "https://not-a-websocket.example".to_owned(),
        );
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("HTTP URL must not be accepted for WebSocket RPC");

        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "SOLANA_RPC_WS_URL",
                ..
            }
        ));

        values.insert(
            "SOLANA_RPC_WS_URL".to_owned(),
            "wss://api.devnet.solana.com".to_owned(),
        );
        values.insert("SOLANA_COMMITMENT".to_owned(), "maybe".to_owned());
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("unknown commitment must fail closed");

        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "SOLANA_COMMITMENT",
                ..
            }
        ));

        values.insert("SOLANA_COMMITMENT".to_owned(), "processed".to_owned());
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("getTransaction cannot use processed commitment");

        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "SOLANA_COMMITMENT",
                ..
            }
        ));
    }

    #[test]
    fn collector_concurrency_and_window_limits_are_bounded() {
        let mut values = base_values();
        values.insert("SOLANA_RPC_MAX_IN_FLIGHT".to_owned(), "129".to_owned());
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("unbounded RPC concurrency must fail closed");
        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "SOLANA_RPC_MAX_IN_FLIGHT",
                ..
            }
        ));

        values.insert("SOLANA_RPC_MAX_IN_FLIGHT".to_owned(), "8".to_owned());
        values.insert(
            "SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND".to_owned(),
            "0".to_owned(),
        );
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("zero discovery request rate must fail closed");
        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND",
                ..
            }
        ));

        values.insert(
            "SOLANA_DISCOVERY_RPC_REQUESTS_PER_SECOND".to_owned(),
            "1".to_owned(),
        );
        values.insert("DISCOVERY_OBSERVATION_WINDOW_MS".to_owned(), "0".to_owned());
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("zero observation window must fail closed");
        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "DISCOVERY_OBSERVATION_WINDOW_MS",
                ..
            }
        ));

        values.insert(
            "DISCOVERY_OBSERVATION_WINDOW_MS".to_owned(),
            "60000".to_owned(),
        );
        values.insert("COLLECTOR_QUEUE_CAPACITY".to_owned(), "100001".to_owned());
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("unbounded collector queues must fail closed");
        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "COLLECTOR_QUEUE_CAPACITY",
                ..
            }
        ));
    }

    #[test]
    fn prefilter_timeout_and_window_relationships_fail_closed() {
        let mut values = base_values();
        values.insert("SOLANA_REQUEST_TIMEOUT_MS".to_owned(), "16000".to_owned());
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("RPC timeout cannot exceed freshness");
        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "SOLANA_REQUEST_TIMEOUT_MS",
                ..
            }
        ));

        values.insert("DISCOVERY_MAX_EVENT_AGE_MS".to_owned(), "30000".to_owned());
        values.insert(
            "DISCOVERY_OBSERVATION_WINDOW_MS".to_owned(),
            "15000".to_owned(),
        );
        let error = Config::from_values(|name| values.get(name).cloned())
            .expect_err("observation window cannot close before RPC timeout");
        assert!(matches!(
            error,
            ConfigError::Invalid {
                name: "DISCOVERY_OBSERVATION_WINDOW_MS",
                ..
            }
        ));
    }
}
