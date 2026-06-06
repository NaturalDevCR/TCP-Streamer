//! Server connection allowlist (IP / CIDR matching), pure & testable.

use ipnet::IpNet;
use std::net::IpAddr;

/// Parses a list of IP or CIDR rules separated by commas, spaces, or newlines.
/// A bare IP (`10.0.0.5`, `fe80::1`) becomes a single-host network. Invalid
/// tokens are skipped.
pub fn parse_rules(s: &str) -> Vec<IpNet> {
    s.split([',', ' ', '\n', '\t', '\r'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .filter_map(|t| match t.parse::<IpNet>() {
            Ok(net) => Some(net),
            Err(_) => t.parse::<IpAddr>().ok().map(IpNet::from),
        })
        .collect()
}

/// Whether `peer` is allowed. Empty `rules` returns `allow_if_empty` (open by
/// default); otherwise true iff some rule contains `peer`.
pub fn is_allowed(peer: IpAddr, rules: &[IpNet], allow_if_empty: bool) -> bool {
    if rules.is_empty() {
        return allow_if_empty;
    }
    rules.iter().any(|net| net.contains(&peer))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn empty_rules_use_default() {
        assert!(is_allowed(ip("10.0.0.1"), &[], true));
        assert!(!is_allowed(ip("10.0.0.1"), &[], false));
    }

    #[test]
    fn exact_ipv4_match() {
        let rules = parse_rules("192.168.1.50, 10.0.0.5");
        assert!(is_allowed(ip("10.0.0.5"), &rules, false));
        assert!(!is_allowed(ip("10.0.0.6"), &rules, false));
    }

    #[test]
    fn cidr_v4_range() {
        let rules = parse_rules("192.168.1.0/24");
        assert!(is_allowed(ip("192.168.1.200"), &rules, false));
        assert!(!is_allowed(ip("192.168.2.1"), &rules, false));
    }

    #[test]
    fn cidr_v6_range() {
        let rules = parse_rules("fe80::/10");
        assert!(is_allowed(ip("fe80::abcd"), &rules, false));
        assert!(!is_allowed(ip("2001:db8::1"), &rules, false));
    }

    #[test]
    fn invalid_tokens_are_skipped() {
        let rules = parse_rules("not-an-ip, 10.0.0.5, , 999.999.0.0/8");
        assert_eq!(rules.len(), 1);
        assert!(is_allowed(ip("10.0.0.5"), &rules, false));
    }
}
