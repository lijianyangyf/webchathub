// src/config.rs

use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_addr: String,   // 监听地址
    pub log_level: String,     // 日志等级
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_addr: "127.0.0.1:9000".to_string(),
            log_level: "info".to_string(),
        }
    }
}

impl Config {
    /// 从环境变量加载配置，未设置则使用默认值
    pub fn from_env() -> Self {
        Self {
            server_addr: env::var("SERVER_ADDR").unwrap_or_else(|_| Self::default().server_addr),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| Self::default().log_level),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server_addr, "127.0.0.1:9000");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_env_config() {
        // 设置临时环境变量
        unsafe {
            env::set_var("SERVER_ADDR", "0.0.0.0:8080");
            env::set_var("LOG_LEVEL", "debug");
        }
        let config = Config::from_env();
        assert_eq!(config.server_addr, "0.0.0.0:8080");
        assert_eq!(config.log_level, "debug");

        // 清理环境变量
        unsafe {
            env::remove_var("SERVER_ADDR");
            env::remove_var("LOG_LEVEL");
        }
    }
}
