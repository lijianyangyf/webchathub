use my_chat::client::ui::start_cli_client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let ws_url_opt = std::env::args().skip(1).next();

    match &ws_url_opt {
        Some(url) => println!("Connecting to server {url} ..."),
        None => println!("Connecting to default ws://127.0.0.1:9000 ..."),
    }

    start_cli_client(ws_url_opt).await
}