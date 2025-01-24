# Pumpfun fork program

## Prerequites

Install Rust, Solana, and Anchor

Here's a useful link. https://anchor-lang.com/docs/installation

```bash
# check rust version
rustc --version

# check solana version
solana --version

# check anchor version
# should be 0.30.1
anchor --version

# check solana configuration
solana config get

# set solana rpc as devnet
solana config set --url devnet

# check wallet set in the config
solana balance

# generate new wallet if doesn't exist
solana-keygen new

# airdrop some devnet SOL
solana airdrop 5
```

Prepare the project
```bash
# clone the git repo
git clone https://github.com/...

# install node modules
yarn
```

## Quick Start

### Build the program

```bash
# build the program
# it will generate new keypair for the program if doesn't exist
# and it will make a build version
anchor build

# sync all keys in program
anchor keys sync

# build again if the program address in lib.rs is changed
anchor build

# you can get keypair and so file here
# ./target/deploy/pumpfun-keypair.json
# ./target/deploy/pumpfun.so
```

### Run tests on localnet

Set the cluster as localnet in `Anchor.toml`:
```bash
[provider]
cluster = "Localnet"
```

you can run the tests without having to start a local network:

```bash
anchor test --provider.cluster Localnet
```

### Test program on devnet

Set the cluster as devnet in `Anchor.toml`:
```bash
[provider]
cluster = "<DEVNET_RPC>"
```

Deploy program:
```bash
anchor deploy
```

#### Use CLI to test the program

Initialize program:
```bash
yarn script config
```

Launch a token:
```bash
yarn script launch
```

Swap SOL for token:
```bash
yarn script swap -t <TOKEN_MINT> -a <SWAP_AMOUNT> -s <SWAP_DIRECTION>
# <TOKEN_MINT>: You can get token mint when you launch a token
# <SWAP_AMOUNT>: SOL/Token amount to swap
# <SWAP_DIRECTION>: 0 - Buy token, 1 - Sell token
```

Migrate token to raydium once the curve is completed:
```bash
yarn script migrate -t <TOKEN_MINT>
# <TOKEN_MINT>: mint address of the token to be launched on the raydium
```
