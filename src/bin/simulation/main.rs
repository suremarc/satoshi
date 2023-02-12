use clap::Parser;
use tokio::{sync::broadcast::error::RecvError, time::Instant};
use tokio_stream::StreamExt;

use crate::net::{
    chain_as_hashes, chain_contains, count_txns, Blockchain, Message, Node, TXN_COUNTER,
};

mod net;

#[derive(Parser, Debug)]
pub struct Args {
    /// How many nodes to simulate
    #[clap(short, long, default_value = "100", value_parser)]
    pub num_nodes: usize,

    // Target time between new blocks
    #[clap(long, default_value = "1.0", value_parser)]
    pub target_block_period_secs: f32,

    // Target number of txns/sec to hit
    #[clap(long, default_value = "2000", value_parser)]
    pub target_txns_per_second: u64,

    /// How many messages to retain
    #[clap(long, default_value = "1000000", value_parser)]
    pub retention: usize,
}

#[tokio::main]
pub async fn main() {
    #[cfg(feature = "tracing")]
    console_subscriber::init(); // needed for tokio profiling

    let args = Args::parse();

    let mean_block_period_secs_per_node = args.target_block_period_secs * (args.num_nodes as f32);
    let txn_period_ms_per_node = (args.num_nodes as u64 * 1000) / args.target_txns_per_second;
    dbg!(mean_block_period_secs_per_node, txn_period_ms_per_node);

    let (tx, mut rx) = tokio::sync::broadcast::channel::<Message>(args.retention);

    for _ in 0..args.num_nodes {
        let tx = tx.clone();
        let node = <Node as Default>::default();
        let mut stream = Box::pin(node.into_producer(
            tx.subscribe(),
            mean_block_period_secs_per_node,
            txn_period_ms_per_node,
        ));
        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                tx.send(msg).unwrap();
            }
        });
    }

    let mut longest_chain = Blockchain::new_sync();
    let mut txns_recorded = 0;
    let mut txns_submitted = 0;
    let mut test_txn = TXN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut test_txn_t_broadcasted = Instant::now();
    tx.send(Message::Txn(test_txn)).unwrap();

    loop {
        match rx.recv().await {
            Ok(Message::Block(blk)) => {
                if blk.len() > longest_chain.len() {
                    longest_chain = blk.clone();

                    let new_txns_recorded = count_txns(&longest_chain) as isize;
                    let txns_recorded_since = new_txns_recorded - txns_recorded;
                    txns_recorded = new_txns_recorded;

                    let new_txns_submitted =
                        TXN_COUNTER.load(std::sync::atomic::Ordering::Relaxed) as isize;
                    let txns_submitted_since = new_txns_submitted - txns_submitted;
                    txns_submitted = new_txns_submitted;

                    println!(
                        "chain: {:x?} ...",
                        chain_as_hashes(&longest_chain).take(4).collect::<Vec<_>>()
                    );
                    println!(
                        "{} blocks total, {} txns in current block, {} new txns on chain, {} new txns submitted",
                        longest_chain.len(),
                        longest_chain.first().unwrap().txns.len(),
                        txns_recorded_since,
                        txns_submitted_since,
                    );

                    if chain_contains(&longest_chain, test_txn) {
                        println!(
                            "transaction latency: {} ms",
                            Instant::now()
                                .duration_since(test_txn_t_broadcasted)
                                .as_millis()
                        );
                        test_txn = TXN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        test_txn_t_broadcasted = Instant::now();
                        tx.send(Message::Txn(test_txn)).unwrap();
                    }
                }
            }
            Err(RecvError::Closed) => break,
            _ => {}
        }
    }
}
