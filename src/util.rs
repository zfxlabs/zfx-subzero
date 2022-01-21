//! Utility functions for consensus algorithms
use std::net::SocketAddr;

use rand::seq::SliceRandom;

use crate::alpha::types::Weight;
use crate::zfx_id::Id;

/// Compute the `hail` consensus weight based on the number of tokens a validator has.
#[inline]
pub fn percent_of(qty: u64, total: u64) -> f64 {
    qty as f64 / total as f64
}

#[inline]
pub fn sum_outcomes(outcomes: Vec<(Id, Weight, bool)>) -> f64 {
    outcomes
        .iter()
        .fold(0.0, |acc, (_id, weight, result)| if *result { acc + *weight } else { acc })
}

#[inline]
pub fn sample_weighted(
    min_w: Weight,
    mut validators: Vec<(Id, SocketAddr, Weight)>,
) -> Option<Vec<(Id, SocketAddr)>> {
    let mut rng = rand::thread_rng();
    validators.shuffle(&mut rng);
    let mut sample = vec![];
    let mut w = 0.0;
    for (id, ip, w_v) in validators {
        if w >= min_w {
            break;
        }
        sample.push((id, ip));
        w += w_v;
    }
    if w < min_w {
        None
    } else {
        Some(sample)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[actix_rt::test]
    async fn test_sampling_insufficient_stake() {
        let dummy_ip: SocketAddr = "0.0.0.0:1111".parse().unwrap();

        let empty = vec![];
        match sample_weighted(0.66, empty) {
            None => (),
            x => panic!("unexpected: {:?}", x),
        }

        let not_enough = vec![(Id::one(), dummy_ip, 0.1), (Id::two(), dummy_ip, 0.1)];
        match sample_weighted(0.66, not_enough) {
            None => (),
            x => panic!("unexpected: {:?}", x),
        }
    }

    #[actix_rt::test]
    async fn test_sampling() {
        let dummy_ip: SocketAddr = "0.0.0.0:1111".parse().unwrap();

        let v = vec![(Id::one(), dummy_ip, 0.7)];
        match sample_weighted(0.66, v) {
            Some(v) => assert!(v == vec![(Id::one(), dummy_ip)]),
            x => panic!("unexpected: {:?}", x),
        }

        let v = vec![(Id::one(), dummy_ip, 0.6), (Id::two(), dummy_ip, 0.1)];
        match sample_weighted(0.66, v) {
            Some(v) => assert!(v.len() == 2),
            x => panic!("unexpected: {:?}", x),
        }

        let v = vec![
            (Id::one(), dummy_ip, 0.6),
            (Id::two(), dummy_ip, 0.1),
            (Id::zero(), dummy_ip, 0.1),
        ];
        match sample_weighted(0.66, v) {
            Some(v) => assert!(v.len() >= 2 && v.len() <= 3),
            x => panic!("unexpected: {:?}", x),
        }
    }

    #[actix_rt::test]
    async fn test_sum_outcomes() {
        let zid = Id::zero();
        let empty = vec![];
        assert_eq!(0.0, sum_outcomes(empty));

        let one_true = vec![(zid, 0.66, true)];
        assert_eq!(0.66, sum_outcomes(one_true));

        let one_false = vec![(zid, 0.66, false)];
        assert_eq!(0.0, sum_outcomes(one_false));

        let true_false =
            vec![(zid, 0.1, false), (zid, 0.1, true), (zid, 0.1, false), (zid, 0.1, true)];
        assert_eq!(0.2, sum_outcomes(true_false));
    }
}
