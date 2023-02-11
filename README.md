# satoshi

This is a simulation of proof-of-work consensus as described in the [Bitcoin whitepaper](https://bitcoin.org/bitcoin.pdf). No effort is given to simulating bad actors, incentive mechanism, or real currency for that matter. We are assuming almost ideal conditions, where every node is honest and the network topology is strongly connected. Messages can certainly be dropped; however, since we simulate a gossip protocol, it's very unlikely for messages not to spread widely.

## How to run

Simply just do `cargo run --release`. That's it. More command-line parameters will come eventually.

## Interpreting the output

This program repeatedly prints three things:

* Most recent 8 hashes on the chain
* Statistics on number of transactions since last block
* Transaction latency (pretty much always equal to time between blocks)

The output looks like this:

```
chain: [2d666b73ece1f692, fad7a37ce726af8f, c2763ec2cf95e335, 9326b5182e580159] ...
88 txns in current block, 88 new txns on chain, 100 new txns submitted
transaction latency: 8 seconds
chain: [e8baab0df0d718c0, 2d666b73ece1f692, fad7a37ce726af8f, c2763ec2cf95e335] ...
73 txns in current block, 73 new txns on chain, 90 new txns submitted
transaction latency: 19 seconds
```

(Note that we abbreviated the chain to 4 elements for the sake of brevity.) In this case, a new block was appended to the existing chain. Sometimes blocks end up being rolled back, though. For example:

```
chain: [64bf1932405df122, 6f012683f35bd24b, 2765ad17bdc3cd8c, a4ee56de2cdc56ea] ...
93 txns in current block, 93 new txns on chain, 100 new txns submitted
transaction latency: 16 seconds
chain: [4216e912495fd300, b0077c0556ae4eb4, 8d87101c8573b22f, 2765ad17bdc3cd8c] ...
130 txns in current block, 129 new txns on chain, 174 new txns submitted
```

`64bf1932405df122` and `6f012683f35bd24b` both disappeared because a longer chain appeared with different blocks. `2765ad17bdc3cd8c` persisted, however.
