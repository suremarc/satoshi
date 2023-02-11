use std::{
    hash::{Hash, Hasher},
    pin::Pin,
    sync::atomic::AtomicU64,
    time::Duration,
};

use async_stream::stream;
use nohash_hasher::IntSet;
use rand::{thread_rng, Rng};
use rpds::ListSync;
use tokio::{sync::mpsc, time::Instant};
use tokio_stream::{Stream, StreamExt, StreamMap};

static TXN_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub enum Message {
    Block(Blockchain),
    Txn(u64),
}

// TODO: actually add real currency to the chain
#[derive(Debug, Clone)]
pub struct Block {
    pub txns: IntSet<u64>,
}

/// Blockchain is an immutable linked list of blocks.
/// Using a fully persistent linked list means we can share optimally,
/// thereby reducing memory consumption of the network as a whole.
/// Though this is not really applicable to a real distributed blockchain.
pub type Blockchain = ListSync<Block>;

pub struct Node {
    pub rx: mpsc::Receiver<Message>,
    pub txn_pool: IntSet<u64>,
    pub current_block: Blockchain,
}

impl Node {
    pub fn new(rx: mpsc::Receiver<Message>) -> Self {
        Self {
            rx,
            txn_pool: Default::default(),
            current_block: Default::default(),
        }
    }

    pub fn check_txns(&self, txns: &IntSet<u64>) -> bool {
        for block in &self.current_block {
            if !txns.is_disjoint(&block.txns) {
                return false;
            }
        }

        true
    }

    pub fn check_txn(&self, txn: u64) -> bool {
        for block in &self.current_block {
            if block.txns.contains(&txn) {
                return false;
            }
        }

        true
    }
}

impl Node {
    pub fn run(&mut self) -> impl Stream<Item = Message> + '_ {
        stream! {
            const T_NEXT_BLOCK: Duration = Duration::from_secs(1);
            const T_NEXT_TXN: Duration = Duration::from_secs(10);

            let block_timer = tokio::time::sleep(T_NEXT_BLOCK+Duration::from_millis(thread_rng().gen_range(0..1_000)));
            tokio::pin!(block_timer);

            let txn_timer = tokio::time::sleep(Duration::from_millis(thread_rng().gen_range(0..1_000)));
            tokio::pin!(txn_timer);

            loop {
                tokio::select! {
                    // Sleeping has completed and a block is ready. Publish the block.
                    _ = block_timer.as_mut() => {
                        self.current_block.push_front_mut(Block {
                            txns: std::mem::replace(&mut self.txn_pool, Default::default())
                        });
                        block_timer.as_mut().reset(Instant::now() + T_NEXT_BLOCK);
                        yield Message::Block(self.current_block.clone());
                    },
                    _ = txn_timer.as_mut() => {
                        txn_timer.as_mut().reset(Instant::now() + Duration::from_millis(thread_rng().gen_range(0..1_000)));
                        yield Message::Txn(TXN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
                    },
                    msg = self.rx.recv() => match msg {
                        // We received a message.
                        Some(Message::Block(block)) => {
                            if block.len() > self.current_block.len() && block.first().map_or(true, |blk| self.check_txns(&blk.txns)) {
                                // A longer chain has been received.
                                // Abandon the current block and start with the new one.
                                self.current_block = block;
                                block_timer.as_mut().reset(Instant::now() + T_NEXT_BLOCK + Duration::from_millis(thread_rng().gen_range(0..1_000)));
                            }
                        },
                        Some(Message::Txn(id)) => {
                            // For now just broadcast it blindly.
                            // TODO: verify that the transaction is valid and has not been double-spent.
                            if self.check_txn(id) && self.txn_pool.insert(id) {
                                yield Message::Txn(id);
                            }
                        }
                        // The channel was closed for some reason.
                        None => return,
                    },
                }
            }
        }
    }
}

fn print_chain(chain: &Blockchain) {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let hashes: Vec<_> = chain
        .iter()
        .map(|block| {
            block.txns.iter().collect::<Vec<_>>().hash(&mut hasher);
            hasher.finish()
        })
        .collect();
    println!("{hashes:x?}")
}

pub async fn run_net(buf_size: usize, n: usize) {
    let (txs, mut nodes): (Vec<_>, Vec<_>) = (0..n)
        .map(|_| mpsc::channel::<Message>(buf_size))
        .map(|(tx, rx)| (tx, Node::new(rx)))
        .unzip();

    let streams: Box<[_]> = nodes.iter_mut().map(Node::run).collect();
    tokio::pin!(streams);

    let mut stream_map: StreamMap<_, _> = streams
        .iter_mut()
        // SAFETY: 'streams' is pinned, so every stream inside 'streams' is also pinned.
        .map(|stream| unsafe { Pin::new_unchecked(stream) })
        .enumerate()
        .collect();

    let mut longest_chain = Blockchain::new_sync();
    while let Some((stream_idx, ref msg)) = stream_map.next().await {
        if let Message::Block(blk) = msg {
            if blk.len() > longest_chain.len() {
                longest_chain = blk.clone();
                print_chain(&longest_chain);
            }
        };

        // this is probably the wrong way to do this
        // it's impossible for there to be enough time for the ingress to be processed;
        // hence, many transactions are dropped
        for (_, tx) in txs.iter().enumerate().filter(|&(i, _)| i == stream_idx) {
            _ = tx.send_timeout(msg.clone(), Duration::from_micros(0)).await;
        }
    }
}
