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
* Time since last block

The output looks like this:

```
chain: [613a7e8cd6c0df3d, b79cf6d529f4bad5, 60ee39bc0dfb041c, bfe6dbc39dc71ef7] ...
56 blocks total, 181 txns in current block, 181 new txns on chain, 260 new txns submitted
period: 4184 ms
chain: [b6c973cb7ab8fe39, 613a7e8cd6c0df3d, b79cf6d529f4bad5, 60ee39bc0dfb041c] ...
57 blocks total, 229 txns in current block, 229 new txns on chain, 323 new txns submitted
period: 5173 ms
```

In this case, a new block was appended to the existing chain. Sometimes blocks end up being rolled back, though. For example:

```
chain: [e2875117dc1a188a, a3cf940b7562e6c, a4c103b81c4c26ec, e698059b8e55cca] ...
67 blocks total, 3 txns in current block, 3 new txns on chain, 266 new txns submitted
period: 5982 ms
chain: [1afb7a7d8f56c713, 4394ae71a036b1cb, a3cf940b7562e6c, a4c103b81c4c26ec] ...
68 blocks total, 280 txns in current block, 491 new txns on chain, 300 new txns submitted
period: 6229 ms
```

`e2875117dc1a188a` disappeared because a longer chain appeared with different blocks. `a3cf940b7562e6c` persisted, however.
