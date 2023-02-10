use std::time::Duration;

use async_stream::stream;
use nohash_hasher::IntSet;
use rpds::ListSync;
use tokio::{sync::mpsc, time::Instant};
use tokio_stream::Stream;

pub enum Message {
    Blk(Blockchain),
    Txn(u64),
}

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
    pub fn run(&mut self) -> impl Stream + '_ {
        stream! {
            const T_NEXT_BLOCK: Duration = Duration::from_secs(1);

            let sleep = tokio::time::sleep(T_NEXT_BLOCK);
            tokio::pin!(sleep);

            loop {
                tokio::select! {
                    // Sleeping has completed and a block is ready. Publish the block.
                    _ = sleep.as_mut() => {
                        self.current_block.push_front_mut(Block {
                            txns: std::mem::replace(&mut self.txn_pool, Default::default())
                        });
                        sleep.as_mut().reset(Instant::now() + T_NEXT_BLOCK);
                        yield Message::Blk(self.current_block.clone());
                    },
                    msg = self.rx.recv() => match msg {
                        // We received a message.
                        Some(msg) => {
                            // TODO: handle the message appropriately.
                            // For now just broadcast it blindly.
                            yield msg;
                        },
                        // The channel was closed for some reason.
                        None => return,
                    },
                }
            }
        }
    }
}
