//! `ISSUBSET` / `ISSUPERSET` for IPv4/IPv6 addresses and CIDR ranges.

use std::net::IpAddr;

/// A parsed network: an address widened to 128 bits, a prefix length, and whether
/// it is IPv6 (so families are never mixed).
struct Network {
    bits: u128,
    prefix: u32,
    is_v6: bool,
}

/// Parse `"addr"` or `"addr/prefix"` into a `Network`. Bare addresses use the full
/// prefix length (32 for v4, 128 for v6).
fn parse_network(s: &str) -> Option<Network> {
    let (addr_part, prefix_part) = match s.split_once('/') {
        Some((a, p)) => (a, Some(p)),
        None => (s, None),
    };
    let addr: IpAddr = addr_part.parse().ok()?;
    match addr {
        IpAddr::V4(v4) => {
            let prefix = match prefix_part {
                Some(p) => p.parse::<u32>().ok().filter(|p| *p <= 32)?,
                None => 32,
            };
            Some(Network {
                bits: u128::from(u32::from(v4)),
                prefix: prefix + 96, // align v4 into the low 32 bits of a 128-bit space
                is_v6: false,
            })
        }
        IpAddr::V6(v6) => {
            let prefix = match prefix_part {
                Some(p) => p.parse::<u32>().ok().filter(|p| *p <= 128)?,
                None => 128,
            };
            Some(Network {
                bits: u128::from(v6),
                prefix,
                is_v6: true,
            })
        }
    }
}

/// Mask `bits` to its top `prefix` bits (out of 128).
fn masked(bits: u128, prefix: u32) -> u128 {
    if prefix == 0 {
        0
    } else if prefix >= 128 {
        bits
    } else {
        let mask = u128::MAX << (128 - prefix);
        bits & mask
    }
}

/// Whether network `a` is entirely contained within network `b`.
fn network_subset(a: &Network, b: &Network) -> bool {
    if a.is_v6 != b.is_v6 {
        return false;
    }
    // `a` is inside `b` only if it is at least as specific and shares b's prefix.
    a.prefix >= b.prefix && masked(a.bits, b.prefix) == masked(b.bits, b.prefix)
}

/// STIX `ISSUBSET`: is the address/range `value` a subset of `range`?
pub fn is_subset(value: &str, range: &str) -> bool {
    match (parse_network(value), parse_network(range)) {
        (Some(a), Some(b)) => network_subset(&a, &b),
        _ => false,
    }
}

/// STIX `ISSUPERSET`: is `value` a superset of `range`? (i.e. `range` ⊆ `value`)
pub fn is_superset(value: &str, range: &str) -> bool {
    is_subset(range, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_address_within_cidr() {
        assert!(is_subset("198.51.100.5", "198.51.100.0/24"));
        assert!(!is_subset("198.51.101.5", "198.51.100.0/24"));
    }

    #[test]
    fn ipv4_cidr_within_cidr() {
        assert!(is_subset("198.51.100.0/25", "198.51.100.0/24"));
        assert!(!is_subset("198.51.100.0/23", "198.51.100.0/24"));
    }

    #[test]
    fn ipv6_within_cidr() {
        assert!(is_subset("2001:db8::1", "2001:db8::/32"));
        assert!(!is_subset("2001:dead::1", "2001:db8::/32"));
    }

    #[test]
    fn mismatched_family_is_not_subset() {
        assert!(!is_subset("198.51.100.5", "2001:db8::/32"));
    }

    #[test]
    fn garbage_is_not_subset() {
        assert!(!is_subset("not-an-ip", "198.51.100.0/24"));
    }

    #[test]
    fn superset_is_inverse() {
        assert!(is_superset("198.51.100.0/24", "198.51.100.5"));
        assert!(!is_superset("198.51.100.5", "198.51.100.0/24"));
    }
}
