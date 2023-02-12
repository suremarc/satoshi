# satoshi

This is a simulation of proof-of-work consensus as described in the [Bitcoin whitepaper](https://bitcoin.org/bitcoin.pdf). No effort is given to simulating bad actors, incentive mechanism, or real currency for that matter. We are assuming almost ideal conditions, where every node is honest and the network topology is strongly connected.

## Usage

The quickest way to get started is to clone this repo and run `cargo run --release` in the root directory.

```shell
Options:
  -n, --num-nodes <NUM_NODES>
          How many nodes to simulate [default: 100]
      --target-block-period-secs <TARGET_BLOCK_PERIOD_SECS>
          [default: 1.0]
      --target-txns-per-second <TARGET_TXNS_PER_SECOND>
          [default: 2000]
      --retention <RETENTION>
          How many messages to retain [default: 1000000]
  -h, --help
          Print help
```

## Interpreting the output

This program repeatedly prints three things:

* Most recent 4 hashes on the chain
* Statistics on number of transactions since last block
* Transaction latency (only is printed once data is available)

The output looks like this:

```
chain: [2d666b73ece1f692, fad7a37ce726af8f, c2763ec2cf95e335, 9326b5182e580159] ...
163 blocks, 88 txns in current block, 88 new txns on chain, 100 new txns submitted
transaction latency: 8787 ms
chain: [e8baab0df0d718c0, 2d666b73ece1f692, fad7a37ce726af8f, c2763ec2cf95e335] ...
164 73 txns in current block, 73 new txns on chain, 90 new txns submitted
transaction latency: 19145 ms
```

In this case, a new block was appended to the existing chain. Sometimes blocks end up being rolled back, though. For example:

```
chain: [64bf1932405df122, 6f012683f35bd24b, 2765ad17bdc3cd8c, a4ee56de2cdc56ea] ...
108 blocks, 93 txns in current block, 93 new txns on chain, 100 new txns submitted
transaction latency: 16480 ms
chain: [4216e912495fd300, b0077c0556ae4eb4, 8d87101c8573b22f, 2765ad17bdc3cd8c] ...
109 blocks, 130 txns in current block, 129 new txns on chain, 174 new txns submitted
```

`64bf1932405df122` and `6f012683f35bd24b` both disappeared because a longer chain appeared with different blocks. `2765ad17bdc3cd8c` persisted, however.
