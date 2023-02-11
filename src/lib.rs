use std::{
    hash::{Hash, Hasher},
    sync::atomic::AtomicU64,
    time::Duration,
};

use async_stream::stream;
use nohash_hasher::IntSet;
use rand::{distributions::Uniform, thread_rng, Rng};
use rpds::ListSync;
use tokio::{
    sync::broadcast::{self, error::RecvError},
    time::Instant,
};
use tokio_stream::{Stream, StreamExt};

static TXN_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
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

pub fn validate_chain(chain: &Blockchain) -> bool {
    let mut set: IntSet<u64> = Default::default();
    for block in chain {
        if set.intersection(&block.txns).next().is_some() {
            return false;
        }

        set.extend(&block.txns);
    }

    true
}

pub struct Node {
    pub rx: broadcast::Receiver<Message>,
    pub txn_pool: IntSet<u64>,
    pub current_block: Blockchain,
}

impl Node {
    pub fn new(rx: broadcast::Receiver<Message>) -> Self {
        Self {
            rx,
            txn_pool: Default::default(),
            current_block: Default::default(),
        }
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
    pub fn rollup_block(&mut self) {
        // add a new transaction to the pool that represents our reward for the block
        self.txn_pool
            .insert(TXN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst));

        // create a new block with all of the transactions in our pool
        self.current_block.push_front_mut(Block {
            txns: std::mem::take(&mut self.txn_pool),
        });
    }

    // Returns true if the new block was accepted, false otherwise.
    pub fn recv_block(&mut self, block: Blockchain) -> bool {
        // Accept this block only if the chain is longer
        // and if
        let cond = block.len() > self.current_block.len() && validate_chain(&block);
        if cond {
            // A longer chain has been received.
            // Abandon the current block and start with the new one.
            self.current_block = block;

            // Remove any transactions from the new chain from our transaction pool.
            for block in self.current_block.iter() {
                self.txn_pool.retain(|txn| block.txns.contains(txn));
            }
        }

        cond
    }

    pub fn run(mut self) -> impl Stream<Item = Message> {
        stream! {
            let next_block_sampler = Uniform::new(Duration::from_secs(5), Duration::from_secs(10));
            let next_txn_sampler = Uniform::new(Duration::ZERO, Duration::from_secs(1));

            // block_timer represents how long it takes to compute a proof of work for our block.
            let block_timer = tokio::time::sleep(thread_rng().sample(next_block_sampler));
            tokio::pin!(block_timer);

            // txn_timer represents how often we initiate transactions on the chain.
            let txn_timer = tokio::time::sleep(thread_rng().sample(next_txn_sampler));
            tokio::pin!(txn_timer);

            loop {
                tokio::select! {
                    _ = block_timer.as_mut() => {
                        // Sleeping has completed and a block is ready. Publish the block.
                        self.rollup_block();
                        block_timer.as_mut().reset(Instant::now() + thread_rng().sample(next_block_sampler));
                        yield Message::Block(self.current_block.clone());
                    },
                    _ = txn_timer.as_mut() => {
                        txn_timer.as_mut().reset(Instant::now() + thread_rng().sample(next_txn_sampler));
                        yield Message::Txn(TXN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
                    },
                    msg = self.rx.recv() => match msg {
                        // We received a message.
                        Ok(Message::Block(block)) => {
                            if self.recv_block(block) {
                                block_timer.as_mut().reset(Instant::now() + thread_rng().sample(next_block_sampler));
                            }
                        },
                        Ok(Message::Txn(id)) => {
                            if self.check_txn(id) && self.txn_pool.insert(id) {
                                yield Message::Txn(id);
                            }
                        }
                        Err(RecvError::Lagged(_)) => continue,
                        // The channel was closed for some reason.
                        Err(RecvError::Closed) => break,
                    },
                }
            }
        }
    }
}

fn chain_as_hashes(chain: &Blockchain) -> impl Iterator<Item = u64> + '_ {
    chain.iter().map(|block| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut txns = block.txns.iter().collect::<Vec<_>>();
        txns.sort();
        txns.hash(&mut hasher);
        hasher.finish()
    })
}

fn count_txns(chain: &Blockchain) -> usize {
    chain.iter().map(|block| block.txns.len()).sum()
}

pub async fn run_net(buf_size: usize, n: usize) {
    let (tx, mut rx) = tokio::sync::broadcast::channel::<Message>(buf_size);

    for _ in 0..n {
        let tx = tx.clone();
        let mut stream = Box::pin(Node::new(tx.subscribe()).run());
        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                tx.send(msg).unwrap();
            }
        });
    }

    let mut longest_chain = Blockchain::new_sync();
    let mut txns_recorded = 0;
    let mut txns_submitted = 0;
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
                        "{:x?}",
                        chain_as_hashes(&longest_chain).take(8).collect::<Vec<_>>()
                    );
                    println!(
                        "{} txns in current block, {} new txns on chain, {} new txns submitted",
                        longest_chain.first().unwrap().txns.len(),
                        txns_recorded_since,
                        txns_submitted_since,
                    );
                }
            }
            Err(RecvError::Closed) => break,
            _ => {}
        }
    }
}
