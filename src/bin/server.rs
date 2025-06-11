// src/bin/server.rs

use my_chat::config::Config;
use my_chat::hub::ChatHub;
use my_chat::server::listener::start_ws_listener;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 加载配置（支持环境变量或默认值）
    let config = Config::from_env();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&config.log_level)).init();
    println!("启动服务器，监听地址：{}", config.server_addr);

    // 创建 hub 命令通道
    let (hub_tx, hub_rx) = mpsc::channel(128);

    // 启动 ChatHub（管理所有房间与广播）
    let mut hub = ChatHub::new(hub_rx);
    tokio::spawn(async move {
        hub.run().await;
    });

    // 启动 WebSocket 监听
    start_ws_listener(&config.server_addr, hub_tx).await
}
