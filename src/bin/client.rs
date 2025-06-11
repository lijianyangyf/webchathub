// src/bin/client.rs

use my_chat::client::ui::start_cli_client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 可通过参数指定 ws 地址，默认 ws://127.0.0.1:9000
    let args: Vec<String> = std::env::args().collect();
    let ws_url = if args.len() > 1 {
        &args[1]
    } else {
        "ws://127.0.0.1:9000"
    };

    println!("正在连接服务器 {} ...", ws_url);
    start_cli_client(ws_url).await
}
