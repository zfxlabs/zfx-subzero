use crate::zfx_id::Id;

use crate::alpha::transfer;

use crate::cell::inputs::Inputs;
use crate::cell::outputs::{Output, Outputs};
use crate::cell::types::*;
use crate::cell::{Cell, CellType};

use super::{constants, Error, Result};

use crate::cell::cell_operation::{consume_from_cell, ConsumeResult};
use ed25519_dalek::Keypair;

/// State of stake assigned to `data` property of [Output]
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct StakeState {
    /// Id of a node which was responsible for staking an account
    pub node_id: Id,
    /// Start time of staking
    pub start_time: u64,
    /// End time of staking
    pub end_time: u64,
}

/// A stake output locks tokens for a specific duration and can be used to stake on the network until
/// the time expires.
pub fn stake_output(
    node_id: Id,
    pkh: PublicKeyHash,
    capacity: Capacity,
    start_time: u64,
    end_time: u64,
) -> Result<Output> {
    let data = bincode::serialize(&StakeState { node_id, start_time, end_time })?;
    Ok(Output { capacity, cell_type: CellType::Stake, data, lock: pkh })
}

/// Creates a stake from [Cell] with indicated capacity for account.
pub struct StakeOperation {
    /// The cell being staked in this staking operation.
    cell: Cell,
    /// The node id of the validator (hash of the TLS certificate (trusted) / ip (untrusted)).
    node_id: Id,
    /// The address which receives the unstaked capacity.
    address: PublicKeyHash,
    /// The amount of capacity to stake.
    capacity: Capacity,
    /// Start time of staking operation
    start_time: u64,
    /// End time of staking operation
    end_time: u64,
}

impl StakeOperation {
    /// Create a stake operation from the provided [Cell] to the new account with `address`.
    /// The method [stake][StakeOperation::stake] should be called to complete the transfer.
    ///
    /// ## Parameters
    /// * `cell` - the requested `capacity` will be taken out from this cell,
    /// if it has outputs with enough balance for the owner with `address`.
    /// * `node_id` - id of a node which stakes the balance.
    /// * `address` - account's public key for whom to stake the balance from `cell`.
    /// * `capacity` - a balance to stake for `address`.
    /// * `staking_start` - start time of staking operation
    /// * `staking_end` - end time of staking operation
    pub fn new(
        cell: Cell,
        node_id: Id,
        address: PublicKeyHash,
        capacity: Capacity,
        staking_start: u64,
        staking_end: u64,
    ) -> Self {
        StakeOperation {
            cell,
            node_id,
            address,
            capacity,
            start_time: staking_start,
            end_time: staking_end,
        }
    }
    /// Stake balance and create a new [Cell] with list of outputs
    /// from the supplied Stake Operation.
    /// In order to construct the new cell with correct list of [inputs][crate::cell::input::Input]
    /// and [outputs][crate::cell::output::Output],
    /// it calls [consume_from_cell][crate::cell::cell_operation::consume_from_cell] to
    /// take out the provided `capacity` from the owner's [outputs][Output] of the cell and
    /// return consumed and remaining balance, as well as the new inputs.
    ///
    /// If the remaining balance has more capacity than [FEE], then
    /// the new cell will have:
    /// * 1 [Output] with the staked balance for the new owner (`address`).
    /// * 1 [Output] with the remaining balance minus [FEE] for the owner (`address`).
    ///
    /// If the remaining balance has less capacity than [FEE], then
    /// only 1 [Output] with the staked balance is returned
    /// for the new owner (`address`).
    ///
    /// ## Parameters
    /// * `keypair` - the account's keypair for identifying outputs for staking.
    pub fn stake(&self, keypair: &Keypair) -> Result<Cell> {
        if self.start_time + constants::STAKING_MIN_DURATION > self.end_time {
            return Err(Error::InvalidStake);
        }
        let ConsumeResult { consumed, residue, inputs } =
            consume_from_cell(&self.cell, self.capacity, keypair)?;

        // Create a change output.
        let main_output = stake_output(
            self.node_id.clone(),
            self.address.clone(),
            consumed,
            self.start_time,
            self.end_time,
        )?;
        let outputs = if residue > FEE && residue - FEE > 0 {
            vec![main_output, transfer::transfer_output(self.address.clone(), residue - FEE)?]
        } else {
            vec![main_output]
        };

        Ok(Cell::new(Inputs::new(inputs), Outputs::new(outputs)))
    }
}

#[cfg(test)]
mod test {
    use super::super::Error;
    use super::*;

    use crate::alpha::coinbase::CoinbaseOperation;

    use crate::alpha::constants;
    use crate::cell::Cell;
    use crate::util;

    use ed25519_dalek::Keypair;

    use std::convert::TryInto;

    #[actix_rt::test]
    async fn test_stake_more_than_allowed_then_throw_error() {
        let staking_start_time = util::get_utc_timestamp_millis();
        let staking_end_time = staking_start_time + constants::STAKING_MIN_DURATION;

        let (kp1, _kp2, _pkh1, pkh2) = generate_keys();

        let c1 = generate_coinbase(&kp1, 1000);
        let stake_op1 = StakeOperation::new(
            c1.clone(),
            Id::generate(),
            pkh2,
            1000,
            staking_start_time,
            staking_end_time,
        );
        let stake_op2 = StakeOperation::new(
            c1,
            Id::generate(),
            pkh2,
            1001 - FEE,
            staking_start_time,
            staking_end_time,
        );
        assert_eq!(stake_op1.stake(&kp1), Err(Error::ExceedsAvailableFunds));
        assert_eq!(stake_op2.stake(&kp1), Err(Error::ExceedsAvailableFunds));
    }

    #[actix_rt::test]
    async fn test_stake_endtime_less_than_two_weeks_then_throw_error() {
        let (kp1, _kp2, pkh1, _pkh2) = generate_keys();

        let staking_start_time = util::get_utc_timestamp_millis();
        let staking_end_time = staking_start_time + constants::STAKING_MIN_DURATION / 2;

        let c1 = generate_coinbase(&kp1, 1000);
        let stake_op = StakeOperation::new(
            c1.clone(),
            Id::generate(),
            pkh1,
            1000 - FEE,
            staking_start_time,
            staking_end_time,
        );
        assert_eq!(stake_op.stake(&kp1), Err(Error::InvalidStake));
    }

    #[actix_rt::test]
    async fn test_stake() {
        let (kp1, _kp2, pkh1, pkh2) = generate_keys();

        let staking_start_time = util::get_utc_timestamp_millis();
        let staking_end_time = staking_start_time + constants::STAKING_MIN_DURATION;

        // Generate a coinbase transaction and stake it
        let c1 = generate_coinbase(&kp1, 1000);
        let stake_op1 = StakeOperation::new(
            c1.clone(),
            Id::generate(),
            pkh2,
            1000 - FEE,
            staking_start_time,
            staking_end_time,
        );
        let c2 = stake_op1.stake(&kp1).unwrap();

        assert_eq!(c2.inputs().len(), 1);
        assert_eq!(c2.outputs().len(), 1);
        // The sum of the outputs should be 900
        assert_eq!(c2.sum(), 1000 - FEE);

        // Stake half the amount in a coinbase tx
        let stake_op2 = StakeOperation::new(
            c1,
            Id::generate(),
            pkh1,
            500,
            staking_start_time,
            staking_end_time,
        );
        let c3 = stake_op2.stake(&kp1).unwrap();
        assert_eq!(c3.inputs().len(), 1);
        assert_eq!(c3.outputs().len(), 2);
        assert_eq!(c3.sum(), 1000 - FEE);
    }

    fn hash_public(keypair: &Keypair) -> [u8; 32] {
        let enc = bincode::serialize(&keypair.public).unwrap();
        blake3::hash(&enc).as_bytes().clone()
    }

    fn generate_coinbase(keypair: &Keypair, amount: u64) -> Cell {
        let pkh = hash_public(keypair);
        let coinbase_op = CoinbaseOperation::new(vec![(pkh, amount)]);
        coinbase_op.try_into().unwrap()
    }

    fn generate_keys() -> (Keypair, Keypair, [u8; 32], [u8; 32]) {
        let kp1_hex = "ad7f2ee3958a7f3fa2c84931770f5773ef7694fdd0bb217d90f29a94199c9d7307ca3851515c89344639fe6a4077923068d1d7fc6106701213c61d34ef8e9416".to_owned();
        let kp2_hex = "5a353c630d3faf8e2d333a0983c1c71d5e9b6aed8f4959578fbeb3d3f3172886393b576de0ac1fe86a4dd416cf032543ac1bd066eb82585f779f6ce21237c0cd".to_owned();

        let kp1 = Keypair::from_bytes(&hex::decode(kp1_hex).unwrap()).unwrap();
        let kp2 = Keypair::from_bytes(&hex::decode(kp2_hex).unwrap()).unwrap();

        let pkh1 = hash_public(&kp1);
        let pkh2 = hash_public(&kp2);

        return (kp1, kp2, pkh1, pkh2);
    }
}
