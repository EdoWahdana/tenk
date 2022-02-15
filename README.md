# Tenk NFT

The main idea of this contract is creating a set of items upfront, for example 10,000, hence tenk.  Then each time a token is minted it is randomly chosen from the remaining tokens. The core mechanism for this is a `Raffle` collection type, which allows randomly picking from a range without replacement. This contract also introduces the idea of using a linkdrop proxy to allow the owner or a normal user to "pre-mint" an item.

## Details

Each `token_id` is numbered in a range, e.g. `0-10,000`.  This each asset and its metadata then named correspondingly, e.g. `0.png`, `0.json`. These files are placed into a flat directory and added to IPFS.  This hash is used as the `base_uri` for the contract and all minted `token_id` can be used to find its corresponding file.

For example,

- [https://bafybeiehqz6vklvxkopg3un3avdtevch4cywuihgxrb4oio2qgxf4764bi.ipfs.dweb.link](https://bafybeiehqz6vklvxkopg3un3avdtevch4cywuihgxrb4oio2qgxf4764bi.ipfs.dweb.link)
- [https://bafybeiehqz6vklvxkopg3un3avdtevch4cywuihgxrb4oio2qgxf4764bi.ipfs.dweb.link/42.png](https://bafybeiehqz6vklvxkopg3un3avdtevch4cywuihgxrb4oio2qgxf4764bi.ipfs.dweb.link/42.png)
- [https://bafybeiehqz6vklvxkopg3un3avdtevch4cywuihgxrb4oio2qgxf4764bi.ipfs.dweb.link/42.json](https://bafybeiehqz6vklvxkopg3un3avdtevch4cywuihgxrb4oio2qgxf4764bi.ipfs.dweb.link/42.json))

## Linkdrop proxy

Currently this project wraps its own linkdrop-proxy, but in the future it this will be its own contract that any contract use for the same ability to add a callback to be used when the linkdrop is claimed. When a linkdrop is created it reserves a raffle draw to be made when claiming. This allows the token to be a surprise (unless it's the last one).

## Development

This project also aims to highlight the newest way to test smart contracts on near using [`near-workspaces`](https://github.com/near/workspaces-js).  See example tests in `__tests__`

## API

Docs are located: `./docs`

Currently there is no standard format to describe the types of a contract. On proposal is to use the (`wit` format)[https://github.com/bytecodealliance/wit-bindgen/blob/main/WIT.md],
which while intended as a tool to generate bindings that act as polyfill for [`WebAssembly Interface Types`](https://github.com/WebAssembly/interface-types), it provides a language agnostic
way to describe types for the API of a Wasm Binary.

This work has led to the creation of [`witme`](https://github.com/ahalabs/witme), a tool for both generating a `.wit` document describing a Rust smart contract and generating a TypeScript file
from a `.wit` document.  The generated TS file also includes a `Contract` class which handles calling the corresponding method.

For example, `nft_transfer` generates the following three functions:

```ts

// will throw if there is an error and parse result if it exist.
nft_transfer(args: {
    receiver_id: AccountId;
    token_id: TokenId;
    approval_id?: u64;
    memo?: string;
}, options?: ChangeMethodOptions): Promise<void>;
nft_transferRaw(args: {
    receiver_id: AccountId;
    token_id: TokenId;
    approval_id?: u64;
    memo?: string;
}, options?: ChangeMethodOptions): Promise<providers.FinalExecutionOutcome>;
nft_transferTx(args: {
    receiver_id: AccountId;
    token_id: TokenId;
    approval_id?: u64;
    memo?: string;
}, options?: ChangeMethodOptions): transactions.Action;
```

### Using the contract's types

The generated typescript can then be compiled to allow for other projects to use.  The main file and types of this package are found `./contracts/tenk/dist/*`
and specified in the `package.json`. These

#### Example

From another TS project:

```ts
import { Contract } from "tenk-nft"

...

const contract = new Contract(account, "tenkv0.testnet.tenk");

async function metadata() {
  const metadata = await contract.nft_metadata();
}
```

## Aspects of Near that prevents hacks on this method of minting

Here is [one example](https://cointelegraph.com/news/85-million-meebits-nft-project-exploited-attacker-nabs-700-000-collectible) of a "hack" that stole $85 million worth of nfts minted in a similar fasion. The "attacker" was able to map the NFT's id (our index) to its worth (its rarity). Then made a contract that made a cross contract call to mint an NFT, then canceling the transaction if it's not rare enough.  Though this cost the "attacker" $20K fees per hour, they were able to see the rare items and reap the reward.

The key aspect that this hack and others like it on Ethereum rely on is that a series of cross contract calls either succeed or fail. This way you can opt out of it before the end and goods never change hands.  On Near this is not the case.  Each cross contract call is asynchronous and can change the state.  This means when you use a cross contract call to mint a token and it succeeds, any money spent is gone and the token minted. Thus unlike the Ethereum example if you aren't satisfied with the token you received you can't choose to not receive it and not pay the owner.


## NFT Standards

For more information about the API provided by the NFT standard see [nomicon.io](https://nomicon.io/Standards/NonFungibleToken).


## Developing

Node must be installed. And Rust must be install see [Getting Started in near-sdk.io](https://www.near-sdk.io/).

To build docs `witme` must be installed.

```bash
cargo install witme
```