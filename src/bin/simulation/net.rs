use std::{
    hash::{Hash, Hasher},
    sync::atomic::AtomicU64,
    time::Duration,
};

use async_stream::stream;
use nohash_hasher::IntSet;
use rand::{distributions::Uniform, thread_rng, Rng};
use rand_distr::Exp;
use rpds::ListSync;
use tokio::{
    sync::broadcast::{self, error::RecvError},
    time::Instant,
};
use tokio_stream::Stream;

// Used for generating unique ID's for transactions.
// Could just as easily be done using UUID's, but
// this also gives us the magical ability to know
// how many transactions have been submitted in total.
pub static TXN_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A block on the chain.
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

/// Check if this chain is valid -- in this case, meaning it has no double-spent transactions.
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

/// Check if this chain has the given transaction.
pub fn chain_contains(chain: &Blockchain, txn: u64) -> bool {
    for block in chain {
        if block.txns.contains(&txn) {
            return true;
        }
    }

    false
}

/// A machine participating in the blockchain network.
#[derive(Default, Debug)]
pub struct Node {
    pub txn_pool: IntSet<u64>,
    pub current_block: Blockchain,
}

/// A message that can be broadcast by a node.
#[derive(Debug, Clone)]
pub enum Message {
    /// Announcing a new block on the chain.
    Block(Blockchain),
    /// Announcing a new transaction, or gossiping a transaction sent from another node.
    Txn(u64),
}

impl Node {
    /// Rollup all of the transactions currently in the pool into a new block,
    /// and append that block to the current chain.
    pub fn rollup_block(&mut self) {
        // add a new transaction to the pool that represents our reward for the block
        self.txn_pool
            .insert(TXN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst));

        // create a new block with all of the transactions in our pool
        self.current_block.push_front_mut(Block {
            txns: std::mem::take(&mut self.txn_pool),
        });
    }

    /// Returns true if the new block was accepted, false otherwise.
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

    /// Returns a stream of messages produced by this node.
    pub fn into_producer(
        mut self,
        mut rx: broadcast::Receiver<Message>,
        mean_block_period_secs: f32,
        txn_period_ms: u64,
    ) -> impl Stream<Item = Message> {
        // The exponential distribution best captures the memorylessness of the proof-of-work computation.
        let next_block_sampler = Exp::new(1. / mean_block_period_secs).unwrap(); // frequency is 1 block / 10 seconds
        let next_txn_sampler =
            Uniform::new(Duration::ZERO, Duration::from_millis(2 * txn_period_ms));

        // block_timer represents how long it takes to compute a proof of work for our block.
        let block_timer = tokio::time::sleep(
            Duration::try_from_secs_f32(thread_rng().sample(next_block_sampler)).unwrap(),
        );

        // txn_timer represents how often we initiate transactions on the chain.
        let txn_timer = tokio::time::sleep(thread_rng().sample(next_txn_sampler));

        stream! {
            tokio::pin!(block_timer);
            tokio::pin!(txn_timer);

            loop {
                tokio::select! {
                    msg = rx.recv() => match msg {
                        // We received a message.
                        Ok(Message::Block(block)) => {
                            if self.recv_block(block) {
                                // We've received a new chain that's longer than ours.
                                block_timer.as_mut().reset(Instant::now() + Duration::try_from_secs_f32(thread_rng().sample(next_block_sampler)).unwrap());
                            }
                        },
                        Ok(Message::Txn(id)) => {
                            // Check if the transaction already exists (avoid double-spending).
                            if !chain_contains(&self.current_block, id) {
                                self.txn_pool.insert(id);
                            }
                        }
                        Err(RecvError::Lagged(_)) => continue,
                        // The channel was closed for some reason.
                        Err(RecvError::Closed) => break,
                    },

                    _ = txn_timer.as_mut() => {
                        txn_timer.as_mut().reset(Instant::now() + thread_rng().sample(next_txn_sampler));
                        yield Message::Txn(TXN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
                    },

                    _ = block_timer.as_mut() => {
                        // Sleeping has completed and a block is ready. Publish the current block and start a new one.
                        self.rollup_block();
                        block_timer.as_mut().reset(Instant::now() + Duration::try_from_secs_f32(thread_rng().sample(next_block_sampler)).unwrap());
                        yield Message::Block(self.current_block.clone());
                    },


                }
            }
        }
    }
}

/// Represent this chain as a list of hashes, mostly for debugging purposes.
pub fn chain_as_hashes(chain: &Blockchain) -> impl Iterator<Item = u64> + '_ {
    chain.iter().map(|block| {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut txns = block.txns.iter().collect::<Vec<_>>();
        txns.sort();
        txns.hash(&mut hasher);
        hasher.finish()
    })
}

pub fn count_txns(chain: &Blockchain) -> usize {
    chain.iter().map(|block| block.txns.len()).sum()
}
