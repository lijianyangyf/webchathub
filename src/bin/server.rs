use std::net::SocketAddr;

use my_chat::config::Config;
use my_chat::hub::ChatHub;
use my_chat::server::listener::start_ws_listener;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::from_env();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&cfg.log_level)).init();
    let hub_tx = ChatHub::spawn();
    let addr: SocketAddr = cfg.server_addr.parse()?;
    start_ws_listener(&addr.to_string(), hub_tx).await
}
