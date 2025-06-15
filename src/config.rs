// src/config.rs – 含历史消息容量 history_limit
// --------------------------------------------------
// 通过环境变量 `HISTORY_LIMIT`（usize）控制，每房间环形缓冲默认 100 条。

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    /// WebSocket 监听地址，如 "127.0.0.1:9000"
    pub server_addr: String,
    /// 日志等级："info" / "debug" 等
    pub log_level: String,
    /// 每个房间保留的历史消息条数（环形缓冲大小）
    pub history_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:9000".into(),
            log_level: "info".into(),
            history_limit: 100,
        }
    }
}

impl Config {
    /// 从环境变量加载配置，未设置字段使用默认值：
    /// * `SERVER_ADDR`
    /// * `LOG_LEVEL`
    /// * `HISTORY_LIMIT`
    pub fn from_env() -> Self {
        let def = Self::default();
        Self {
            server_addr: env::var("SERVER_ADDR").unwrap_or(def.server_addr),
            log_level: env::var("LOG_LEVEL").unwrap_or(def.log_level),
            history_limit: env::var("HISTORY_LIMIT")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(def.history_limit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.server_addr, "127.0.0.1:9000");
        assert_eq!(cfg.log_level, "info");
        assert_eq!(cfg.history_limit, 100);
    }

    #[test]
    fn env_override() {
        let _guard = EnvGuard::set(vec![
            ("SERVER_ADDR", "0.0.0.0:8080"),
            ("LOG_LEVEL", "debug"),
            ("HISTORY_LIMIT", "256"),
        ]);

        let cfg = Config::from_env();
        assert_eq!(cfg.server_addr, "0.0.0.0:8080");
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.history_limit, 256);
    }

    /// RAII 环境变量守卫，测试结束后自动清理。
    struct EnvGuard {
        keys: Vec<&'static str>,
    }

    impl EnvGuard {
        fn set(pairs: Vec<(&'static str, &'static str)>) -> Self {
            for (k, v) in &pairs {
                unsafe{env::set_var(k, v);}
            }
            Self {
                keys: pairs.into_iter().map(|(k, _)| k).collect(),
            }
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
