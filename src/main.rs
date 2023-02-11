use satoshi::run_net;

#[tokio::main]
async fn main() {
    run_net(100, 1000).await
}
