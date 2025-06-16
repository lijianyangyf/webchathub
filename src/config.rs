// src/config.rs – application runtime configuration
// --------------------------------------------------
// * `history_limit`   – ring‑buffer size per room (1‑B)
// * `room_ttl_secs`  – TTL for empty rooms (1‑C)
//
// All values can be overridden via environment variables as documented below.

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    /// WebSocket listen address, e.g. "0.0.0.0:9000"
    pub server_addr: String,
    /// Log level: trace|debug|info|warn|error
    pub log_level: String,
    /// Messages kept per room (history replay)
    pub history_limit: usize,
    /// Seconds before an empty room is garbage‑collected
    pub room_ttl_secs: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_addr: "0.0.0.0:9000".into(),
            log_level: "info".into(),
            history_limit: 100,
            room_ttl_secs: 300, // 5 minutes
        }
    }
}

impl Config {
    /// Load from environment variables with fallback to defaults.
    ///
    /// | Env Var          | Type  | Default | Description                     |
    /// |------------------|-------|---------|---------------------------------|
    /// | `SERVER_ADDR`    | str   | see default | bind address                |
    /// | `LOG_LEVEL`      | str   | "info" | log verbosity                  |
    /// | `HISTORY_LIMIT`  | usize | 100     | per‑room history size          |
    /// | `ROOM_TTL_SECS`  | u64   | 300     | seconds to keep empty rooms    |
    pub fn from_env() -> Self {
        let def = Self::default();
        Self {
            server_addr: env::var("SERVER_ADDR").unwrap_or(def.server_addr),
            log_level: env::var("LOG_LEVEL").unwrap_or(def.log_level),
            history_limit: env::var("HISTORY_LIMIT")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(def.history_limit),
            room_ttl_secs: env::var("ROOM_TTL_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(def.room_ttl_secs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.server_addr, "0.0.0.0:9000");
        assert_eq!(cfg.log_level, "info");
        assert_eq!(cfg.history_limit, 100);
        assert_eq!(cfg.room_ttl_secs, 300);
    }

    #[test]
    fn env_override() {
        let _guard = EnvGuard::set(vec![
            ("SERVER_ADDR", "127.0.0.1:8080"),
            ("LOG_LEVEL", "debug"),
            ("HISTORY_LIMIT", "256"),
            ("ROOM_TTL_SECS", "600"),
        ]);

        let cfg = Config::from_env();
        assert_eq!(cfg.server_addr, "127.0.0.1:8080");
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.history_limit, 256);
        assert_eq!(cfg.room_ttl_secs, 600);
    }

    /// Simple RAII env guard for tests
    struct EnvGuard {
        keys: Vec<&'static str>,
    }

    impl EnvGuard {
        fn set(pairs: Vec<(&'static str, &'static str)>) -> Self {
            for (k, v) in &pairs {
                unsafe{env::set_var(k, v);}
            }
            Self { keys: pairs.into_iter().map(|(k, _)| k).collect() }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for k in &self.keys {
                unsafe{env::remove_var(k);}
            }
        }
    }
}
