use satoshi::run_net;

#[tokio::main]
async fn main() {
    run_net(10_000, 1_000_000).await
}
