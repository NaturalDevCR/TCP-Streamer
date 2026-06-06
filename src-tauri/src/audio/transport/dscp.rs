//! DSCP (Differentiated Services) strategy → IP TOS byte mapping.
//!
//! These keys are the single source of truth shared by the frontend dropdown
//! and the socket setup. Keys are case-insensitive. Unknown keys fall back to
//! best-effort (0x00) rather than failing.

/// Maps a DSCP strategy key to its IP TOS/DSCP byte value.
pub fn dscp_to_tos(strategy: &str) -> u8 {
    match strategy.to_ascii_lowercase().as_str() {
        "voip" | "ef" => 0xB8,      // Expedited Forwarding (DSCP 46)
        "cs5" => 0xA0,              // Class Selector 5 (DSCP 40)
        "lowdelay" => 0x10,         // Legacy "minimize delay"
        "throughput" => 0x08,       // Legacy "maximize throughput"
        "besteffort" | "" => 0x00,  // Best effort / default
        _ => 0x00,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voip_and_ef_map_to_expedited_forwarding() {
        assert_eq!(dscp_to_tos("voip"), 0xB8);
        assert_eq!(dscp_to_tos("ef"), 0xB8);
    }

    #[test]
    fn keys_are_case_insensitive() {
        assert_eq!(dscp_to_tos("VoIP"), 0xB8);
        assert_eq!(dscp_to_tos("LowDelay"), 0x10);
    }

    #[test]
    fn cs5_lowdelay_throughput() {
        assert_eq!(dscp_to_tos("cs5"), 0xA0);
        assert_eq!(dscp_to_tos("lowdelay"), 0x10);
        assert_eq!(dscp_to_tos("throughput"), 0x08);
    }

    #[test]
    fn besteffort_empty_and_unknown_are_zero() {
        assert_eq!(dscp_to_tos("besteffort"), 0x00);
        assert_eq!(dscp_to_tos(""), 0x00);
        assert_eq!(dscp_to_tos("nonsense"), 0x00);
    }
}
