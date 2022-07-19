# (zfx-subzero) Integration Proposal


## Integration with Tezos

### Analysis

The milestone #4 deliverables are modified from the original SoW in order to prioritise bridging between a network with another. The current `deku` solution uses a Tendermint light-client which communicates with a network running Tendermint consensus in order to obtain fraud proofs when an invalid state transition occurs.

There are several aspects to `deku` which can usefully be re-used in a proposed solution since it provides infrastructure for creating and deploying smart contracts and implements interoperability primitives between Tezos nodes and other languages such as `JavaScript`.

However our proposed solution differs from `deku` in how a subnetwork and its committee are specified and in the type of subnetwork which executes alongside `Tezos`. `deku` by default has a fixed initial committee of validators which modifies itself using proof of authority. Instead we propose to use on-chain baking information in order to provide sybil resistance to committee members.

### Proposed Solution

We propose to create a bridge between `Tezos` and `subzero` by defining a validation committee within a `Tezos` smart contract. This allows for a `Tezos` client to verify that a block in a `subzero` network is signed by the committee members specified by the smart contract. 

We propose to create a committee smart contract which allows for the registration of validators based on being bakers within `Tezos`. This allows existing `Tezos` bakers to opt-in to subnetworks of interest in order to secure them. In order to assess whether an account is eligible for registration we propose to use the `VOTING_POWER` michelson instruction for checking that a baker has a stake greater than 6000 XTZ. This effectively permanently records interest from a baker in validating on the subnetwork.

Since baking is dynamic in `Tezos` and bakers are free to unbond their stake at any time, the `subnetwork` validation committee inherits these same properties. This implies a solution which takes into account smart contract operations but also on-chain baking activity implied by the `Tezos` `alpha` protocol. 

As such, a committee co-ordinator program must take into account both types of operations and relay them to the `subnetwork`. Similarly when value is redeemed from the `subnetwork` back into `Tezos`, co-ordinators must take into account both types of on-chain operations.

The core purpose of a committee registration is to bind the TLS certificate public key used in a `subnetwork` to a baker account address. The baker account gives weight (sybil resistance) to the validator in the subnetwork whilst the TLS certificate allows the subnetwork to limit network connections to active bakers.

### Smart Contract State

The smart contract should have three states to help with bootstrapping: 
* `Genesis` - The initialisation state of a subnetwork committee.
* `Sealed` - The acceptation state of a genesis committee, accepted by the smart contract owner.
* `Open` - The committee is open to new registrations from public bakers.

This allows some time for members of the committee to prepare to bootstrap the subnetwork and prevents further subscriptions whilst the subnetwork is initialising.

### Smart Contract Operations

#### Register

A `Tezos` baker should be able to `register` by submitting a `Register` operation to a `Tezos` smart contract in order to be added to a `subzero` committee.

```ocaml
type Register := {
  (* Hash of the TLS public key certificate used to connect to peers on `subzero` *)
  xid : XId,
  (* The identity of the Tezos baking account *)
  baking_account : Public_key_hash,
  (* Subnetwork specific signing public key *)
  public_key : Public_key,
  (* Subnetwork threshold key *)
  threshold_public_key : Public_key option,
}
```

* The smart contract should ensure that the `baking_account` has sufficient voting power to participate (>6,000XTZ).
* The validator submitting this operation *must* ensure that the TLS certificate public key is valid - the smart contract is not expected to check this - an erroneous TLS public key would result in a validator being unable to participate in consensus.
* The validator submitting this operation *must* ensure that the threshold public key is valid, similar to the TLS public key.

#### Transfer

A `Tezos` participant should be able to `transfer` a quantity of `token` (initially `XTZ`) by submitting a `Transfer` operation to a `Tezos` smart contract in order for it to become accessible on the `subzero` network as stake.

```ocaml
type Transfer := {
  (* Type of transfer IN | OUT *)
  transfer_type : Transfer_type,
  (* Transfer recipient (source for out, destination for in) *)
  recipient : Public_key_hash,
  (* Amount of currency to transfer *)
  amount : Qty,
}
```

* The smart contract should ensure that in the case of outbound (from `Tezos` to `Subzero`) transfers, the sender has sufficient `XTZ` available.
* The smart contract should ensure in the case of inbound transfers, that the `recipient` and the operation signer are the same.

### Operation Co-ordination

#### Committee Co-ordinator

A `Tezos` client program should monitor `Tezos` heads, filter for operations which pertain to the committee smart contract and forward these to the subzero `primary` protocol service. Changes in baking state which occur to accounts registered within a committee should be relayed to `subzero`. This allows for removing bakers which stop baking on `Tezos`.

The client program:
* Listens for `Register` and `Transfer` operations committed to the `Tezos` blockchain and sends respective `Register` and `Transfer` `subzero` operations which are encoded as `Rust` decodable data to a consensus mempool worker.
* Implements functions which serialize `Register` and `Transfer` subzero operations into `subzero` operations and are callable from `OCaml`.
* The `co-ordinator` should ensure that operations sent to it are final before committing them to the `primary` protocol.

#### Subzero `primary` protocol service

A primary protocol with support for `Register` and `Transfer` operations should be applied to the `subzero` network state. Note that this work is partially complete, missing changes relating to the integration with the `Tezos` committee co-ordinator.

Initially at bootstrap:
* Reads all existing known committee operations committed to the `Tezos` blockchain and synchronises the data according to the `subzero` `primary` protocol store.
* Triggers an `ice` bootstrap which initialises the network committee metadata and begins recording validator uptimes. This is required in order for consensus to be aware of network endpoints.
* Triggers a `consensus` bootstrap which initialises the committee weight metadata. This is required in order for consensus to assign weight to committee members during validation.

Once the protocol service is running:
* Receives blocks from `consensus` and applies the (totally) ordered operations to the state.
* Blocks are tagged with a quorum certificate which contain the signature required for supplying blocks to a `Tezos` client (the bridging endpoint).

### Integration into `ice` (bootstrap)

The smart contract committee co-ordinator should provide validator identities to `ice` at the `subzero` network bootstrapping stage and subsequently update `ice` with changes in the validator set. This allows validators in `subzero` to connect to one another through the `ice` peer to peer layer and allows for subnetworks to determine the relative liveness of peers.

The integration program:
* Bootstraps `ice` from the existing operations - note: `ice` has to be provided with network endpoints along with the identities `Id` designated in the stake operations so that it knows how to connect to the nodes securely.
* `ice` maintains a notion of `liveness` for the validators validating the network.
* When a sufficient degree (`2f+1`) of the networks validator set is `live`, the consensus is provided with a `LiveCommittee` in order to start or resume consensus.
* When `f` validators or more become `Faulty`, `ice` should provide consensus with `FaultyCommittee` in order to pause block based consensus.
* When a specific validator obtains or loses liveness, the `primary` protocol service is notified and persists a notion of `uptime` relating to the validating committee.
