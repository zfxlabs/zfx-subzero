# (zfx-subzero) Integration Proposal

## Integration to Tezos

### Analysis

The milestone #4 deliverables are modified from the original SoW in order to prioritise bridging between a network with another. The current `deku` solution uses a Tendermint light-client which communicates with a network running Tendermint consensus in order to obtain fraud proofs when an invalid state transition occurs.

### Proposed solution

We propose to create a bridge between `Tezos` and `subzero` by defining the validation committee within a `Tezos` smart contract. This allows for a `Tezos` client to later verify that a block in `subzero` was signed by a correct committee (specified by the smart contract).

The smart contract should allow registration of validators by specifying up to three distinct types of keys. Note that threshold signatures are only required with a solution which is not based on sortition for selecting proposers and we may opt to omit this field if sortition proves sufficient.
1. The public key of a TLS certificate representing the network identity of a validator for identification at the network level.
2. The public key of a private key used for signing blocks.
3. (optional) The public key of a private key used for the formation of threshold signatures.

The smart contract will allow validators to lock `stake` for a duration larger than 2 weeks and smaller or equal to one year. This is required for weighting the validator set in `subzero`. Initially the team plans to use testnet `XTZ` and later form an economic strategy based on feedback from members of the `Tezos` community.

The committee smart contract is expected to have an owner which, in the case of threshold signature registration acts as a trusted dealer. With a trustless setup, it is expected the owner may relinquish ownership of the contract.

### Smart Contract Operations

#### Stake


A subzero validator should be able to `stake` by submitting a `Stake` operation to a `Tezos` smart contract in order to be added to the `subzero` committee.

```ocaml
type Stake := {
  (* Hash of the TLS public key certificate used to connect to peers on `subzero` *)
  id : Id,
  (* Subzero threshold key *)
  threshold_public_key : Public_key,
  (* Subzero / Tezos signing key public key *)
  public_key : Public_key,
  (* The time that this validator wishes to stake *)
  start_time : Timestamp,
  (* The time that this validator wishes to finish staking *)
  end_time : Timestamp,
  (* The amount that the validator wishes to lock as stake *)
  amount : Qty,
}
```

* The smart contract should ensure that the `start_time` is later than now by at least `1 minute`.
* The smart contract should ensure that the `end_time` is at least 2 weeks after `start_time` and at most 1 year after the `start_time`.
* The smart contract should ensure that the `amount` staked is greater than or equal to `1000 XTZ`.
* The smart contract should check that the `public_key` corresponds with the operations signing key.
* The validator submitting this operation *must* ensure that the TLS certificate public key is valid - the smart contract is not expected to check this - an erroneous TLS public key would result in a validator being unable to participate in consensus.
* The validator submitting this operation *must* ensure that the threshold public key is valid, similar to the TLS public key.

#### Transfer

A `Tezos` participant should be able to `transfer` a quantity of `token` by submitting a `Transfer` operation to a `Tezos` smart contract in order for it to become accessible on the `subzero` network as stake.

```ocaml
type Transfer := {
  (* Type of transfer IN | OUT *)
  transfer_type : Transfer_type,
  (* Transfer recipient (src for out, dest for in) *)
  recipient : Public_key_hash,
  (* Amount of currency to transfer *)
  amount : Qty,
}
```

* The smart contract should ensure that in the case of outbound (from `Tezos` to `Subzero`) transfers, the sender has sufficient `XTZ` available.
* The smart contract should ensure in the case of inbound transfers, that the `recipient` and the operation signer are the same.

### Operation Co-ordination

#### Committee Co-ordinator

A `Tezos` client program should monitor `Tezos` heads, filter for operations which pertain to the committee smart contract and forward these to the subzero `primary` protocol service.

The client program:
* Listens for `Stake` and `Transfer` operations committed to the `Tezos` blockchain and sends respective `Stake` and `Transfer` `subzero` operations which have been encoded as `Rust` decodable data to the consensus mempool worker.
* Implements Rust foreign functions which serialize `Stake` and `Transfer` subzero operations callable from `OCaml`.
* Note: The `co-ordinator` should ensure that enough blocks have passed such that the operations sent to it are final.

#### Subzero `primary` protocol service

A primary protocol with support for `Stake` and `Transfer` operations should be applied to the `subzero` network state. Note that this work is partially complete, missing changes relating to the integration with the `Tezos` committee co-ordinator.

Initially at bootstrap:
* Reads all existing known committee operations committed to the `Tezos` blockchain and synchronises the data according to the `subzero` `primary` protocol store.
* Triggers a `consensus` bootstrap which initialises the network committee metadata. This is required in order for consensus to be aware of network endpoints.

Once the protocol service is running:
* Receives blocks from `consensus` and applies the (totally) ordered operations to the state.
* Blocks are tagged with a quorum certificate which contain the signature required for supplying blocks to a Tezos client (the bridging endpoint).
