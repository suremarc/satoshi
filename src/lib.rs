use std::{pin::Pin, time::Duration};

use async_stream::stream;
use nohash_hasher::IntSet;
use rpds::ListSync;
use tokio::{sync::mpsc, time::Instant};
use tokio_stream::{Stream, StreamExt, StreamMap};

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
}

impl Node {
    pub fn run(&mut self) -> impl Stream<Item = Message> + '_ {
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
                        yield Message::Block(self.current_block.clone());
                    },
                    msg = self.rx.recv() => match msg {
                        // We received a message.
                        // TODO: verify that every transaction is valid and has not been double-spent.
                        Some(Message::Block(block)) => {
                            if self.current_block.len() < block.len() {
                                // A longer chain has been received.
                                // Abandon the current block and start with the new one.
                                self.current_block = block;
                                sleep.as_mut().reset(Instant::now() + T_NEXT_BLOCK);
                            }
                        },
                        Some(Message::Txn(id)) => {
                            // For now just broadcast it blindly.
                            // TODO: verify that the transaction is valid and has not been double-spent.
                            if self.txn_pool.insert(id) {
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

pub async fn run_net(buf_size: usize, n: usize) {
    let (txs, nodes): (Vec<_>, Vec<_>) = (0..n)
        .map(|_| mpsc::channel::<Message>(buf_size))
        .map(|(tx, rx)| (tx, Node::new(rx)))
        .unzip();

    let mut nodes = nodes.into_boxed_slice();

    let streams: Box<[_]> = nodes.iter_mut().map(Node::run).collect();
    tokio::pin!(streams);

    let mut stream_map: StreamMap<_, _> = streams
        .iter_mut()
        // SAFETY: 'streams' is pinned, so every stream inside 'streams' is also pinned.
        .map(|stream| unsafe { Pin::new_unchecked(stream) })
        .enumerate()
        .collect();

    while let Some((stream_idx, ref msg)) = stream_map.next().await {
        match msg {
            Message::Block(blk) => println!("{:?}", blk.first()),
            Message::Txn(id) => println!("transaction: {id}"),
        };

        for (_, tx) in txs.iter().enumerate().filter(|&(i, _)| i == stream_idx) {
            _ = tx.send_timeout(msg.clone(), Duration::from_micros(0)).await;
        }
    }
}
