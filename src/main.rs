use satoshi::run_net;

#[tokio::main]
async fn main() {
    run_net(1_000_000, 1000).await
}
