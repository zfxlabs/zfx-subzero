use zfx_sortition::sortition;

use crate::alpha::types::{VrfOutput, Weight};
use crate::util;
use crate::zfx_id::Id;

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;

use tracing::*;

use crate::colored::Colorize;

type StakingCapacity = u64;

pub struct Committee {
    self_id: Id,
    self_staking_capacity: u64,
    /// The list of validators and allocations which formed this committee.
    validators: HashMap<Id, (SocketAddr, StakingCapacity)>,
    /// The validating committee.
    committee: HashMap<Id, (SocketAddr, Weight)>,
    /// The block production vrf (a vrf for `height + 1` if we are the next block producer).
    block_production_slot: Option<VrfOutput>,
    /// Whether we have already proposed a block at this height.
    block_proposed: bool,
    block_producers: HashSet<VrfOutput>,
}

impl std::ops::Deref for Committee {
    type Target = HashMap<Id, (SocketAddr, Weight)>;

    fn deref(&self) -> &'_ Self::Target {
        &self.committee
    }
}

fn compute_vrf_h(id: Id, vrf_out: &VrfOutput) -> [u8; 32] {
    let vrf_h = vec![id.as_bytes(), vrf_out].concat();
    blake3::hash(&vrf_h).as_bytes().clone()
}

impl Committee {
    pub fn empty(self_id: Id) -> Self {
        Committee {
            self_id,
            self_staking_capacity: 0u64,
            validators: HashMap::default(),
            committee: HashMap::default(),
            block_production_slot: None,
            block_proposed: false,
            block_producers: HashSet::default(),
        }
    }

    fn calculate_total_staking_capacity(&self, init: StakingCapacity) -> StakingCapacity {
        let mut total_staking_capacity = init;
        for (_, (_, staking_capacity)) in self.validators.iter() {
            total_staking_capacity += staking_capacity;
        }
        info!("[{}] total_staking_capacity = {:?}", "committee".yellow(), total_staking_capacity);
        total_staking_capacity
    }

    pub fn next_committee(
        &mut self,
        vrf_output: VrfOutput,
        validators: HashMap<Id, (SocketAddr, StakingCapacity)>,
    ) -> (HashMap<Id, (SocketAddr, Weight)>, HashSet<VrfOutput>, Option<VrfOutput>) {
        let expected_size = (validators.len() as f64).sqrt().ceil() + 100.0;
        info!("[{}] expected_size = {:?}", "committee".yellow(), expected_size);

        let total_staking_capacity =
            self.calculate_total_staking_capacity(self.self_staking_capacity);

        let mut committee = HashMap::default();
        let mut block_producers = HashSet::new();
        for (id, (ip, staking_capacity)) in validators.iter() {
            let vrf_h = compute_vrf_h(id.clone(), &vrf_output);
            let s_w =
                sortition::select(*staking_capacity, total_staking_capacity, expected_size, &vrf_h);
            // If the sortition weight > 0 then this `id` is a block producer.
            if s_w > 0 {
                block_producers.insert(vrf_h.clone());
            }
            info!("percent_of {:?}, total = {:?}", *staking_capacity, total_staking_capacity);
            let v_w = util::percent_of(*staking_capacity, total_staking_capacity);
            if let Some(_) = committee.insert(id.clone(), (ip.clone(), v_w)) {
                panic!("duplicate validator insertion");
            } else {
                info!("inserted validator = {} with weight = {:?}", id.clone(), v_w);
            }
        }

        // Compute whether we are a block producer
        let mut block_production_slot = None;
        let vrf_h = compute_vrf_h(self.self_id.clone(), &vrf_output);
        let s_w = sortition::select(
            self.self_staking_capacity,
            total_staking_capacity,
            expected_size,
            &vrf_h,
        );
        if s_w > 0 {
            block_producers.insert(vrf_h.clone());
            block_production_slot = Some(vrf_h.clone());
        }

        info!(
            "[{}] is_block_producer = {:?}",
            "committee".yellow(),
            block_production_slot.is_some()
        );

        (committee, block_producers, block_production_slot)
    }

    pub fn next(
        &mut self,
        self_staking_capacity: u64,
        vrf_output: VrfOutput,
        validators: HashMap<Id, (SocketAddr, StakingCapacity)>,
    ) {
        self.self_staking_capacity = self_staking_capacity;
        self.validators = validators.clone();
        let (committee, block_producers, block_production_slot) =
            self.next_committee(vrf_output, validators);
        self.committee = committee;
        self.block_producers = block_producers;
        self.block_production_slot = block_production_slot;
        self.block_proposed = false;
    }

    #[allow(unused)] // Currently not used
    pub fn is_valid_vrf(&self, vrf_output: VrfOutput) -> bool {
        self.block_producers.contains(&vrf_output)
    }

    pub fn block_production_slot(&self) -> Option<VrfOutput> {
        self.block_production_slot.clone()
    }

    pub fn self_staking_capacity(&self) -> StakingCapacity {
        self.self_staking_capacity.clone()
    }

    pub fn validators(&self) -> HashMap<Id, (SocketAddr, StakingCapacity)> {
        self.validators.clone()
    }

    pub fn block_proposed(&self) -> bool {
        self.block_proposed
    }

    pub fn set_block_proposed(&mut self, proposed: bool) {
        self.block_proposed = proposed;
    }
}
