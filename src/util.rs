//! Utility functions for consensus algorithms
use std::net::{SocketAddr, ToSocketAddrs};

use chrono::{DateTime, TimeZone, Utc};
use rand::seq::SliceRandom;

use crate::alpha::types::Weight;
use crate::cell::{Cell, CellType};
use crate::zfx_id::Id;
use crate::{Error, Result};

/// Compute the `hail` consensus weight based on the number of tokens a validator has.
#[inline]
pub fn percent_of(qty: u64, total: u64) -> f64 {
    qty as f64 / total as f64
}

/// Sum the positive query outcomes by weight
#[inline]
pub fn sum_outcomes(outcomes: Vec<(Id, Weight, bool)>) -> f64 {
    outcomes
        .iter()
        .fold(0.0, |acc, (_id, weight, result)| if *result { acc + *weight } else { acc })
}

/// Sample the required weight from a list of validators
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

/// Gets system clock in millisec sicne unix epoch
pub fn get_utc_timestamp_millis() -> u64 {
    Utc::now().timestamp_millis() as u64
}

/// Converts timestamp in millisec to DateTime UTC
pub fn from_ts_millis(ts: u64) -> DateTime<Utc> {
    Utc.timestamp((ts / 1_000) as i64, (ts % 1000) as u32 * 1_000_000)
}

/// Converts DateTime UTC to timestamp in millisec
pub fn to_ts(time: DateTime<Utc>) -> u64 {
    time.timestamp_millis() as u64
}

/// Parse a peer description from the format `IP` or `ID@IP` to its ID and address
pub fn parse_id_and_ip(s: &str) -> Result<(Id, SocketAddr)> {
    let parts: Vec<&str> = s.split('@').collect();
    if parts.len() == 1 {
        let ip: SocketAddr =
            parts[0].to_socket_addrs().map_err(|_| Error::PeerParseError)?.next().unwrap();
        let id = Id::from_ip(&ip);
        Ok((id, ip))
    } else if parts.len() == 2 {
        let id: Id = parts[0].parse().map_err(|_| Error::PeerParseError)?;
        let ip: SocketAddr =
            parts[1].to_socket_addrs().map_err(|_| Error::PeerParseError)?.next().unwrap();
        Ok((id, ip))
    } else {
        Err(Error::PeerParseError)
    }
}

/// Check if a cell creates a coinbase output.
pub fn has_coinbase_output(cell: &Cell) -> bool {
    for o in cell.outputs().iter() {
        if o.cell_type == CellType::Coinbase {
            return true;
        }
    }
    false
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

    #[actix_rt::test]
    async fn test_parse_id_and_ip() {
        // ID and IP
        let id = Id::zero();
        let ip_str = "0.0.0.0:1111";
        let addr: SocketAddr = ip_str.parse().unwrap();
        let peer_str = format!("{}@{}", id, ip_str);
        let (id2, addr2) = parse_id_and_ip(&peer_str).unwrap();
        assert_eq!(id, id2);
        assert_eq!(addr, addr2);

        let id = Id::new(b"to_be_hashed");
        let ip_str = "1.2.3.4:5678";
        let addr: SocketAddr = ip_str.parse().unwrap();
        let peer_str = format!("{}@{}", id, ip_str);
        let (id2, addr2) = parse_id_and_ip(&peer_str).unwrap();
        assert_eq!(id, id2);
        assert_eq!(addr, addr2);

        // IP-only
        let ip_str = "0.0.0.0:1111";
        let addr: SocketAddr = ip_str.parse().unwrap();
        let peer_str = format!("{}", ip_str);
        let (_id2, addr2) = parse_id_and_ip(&peer_str).unwrap();
        assert_eq!(addr, addr2);

        // Errors
        match parse_id_and_ip("") {
            Err(Error::PeerParseError) => (),
            other => panic!("Unexpected {:?}", other),
        }

        match parse_id_and_ip("@") {
            Err(Error::PeerParseError) => (),
            other => panic!("Unexpected {:?}", other),
        }

        match parse_id_and_ip("@1.2.3.4:5678") {
            Err(Error::PeerParseError) => (),
            other => panic!("Unexpected {:?}", other),
        }

        match parse_id_and_ip("not-an-id@0.0.0.0:1111") {
            Err(Error::PeerParseError) => (),
            other => panic!("Unexpected {:?}", other),
        }

        let id = Id::new(b"to_be_hashed");
        let peer_str = format!("{}@not-an-ip", id);
        match parse_id_and_ip(&peer_str) {
            Err(Error::PeerParseError) => (),
            other => panic!("Unexpected {:?}", other),
        }
    }
}
