//! Every `AgentKind` round-trips through its `u8` representation (bus wire format), so a new
//! agent cannot be silently decoded as `GenericMcp` (PR #27 review).
use icx_core::AgentKind;

#[test]
fn every_agent_kind_round_trips_through_its_repr() {
    for kind in AgentKind::ALL {
        assert_eq!(AgentKind::from_repr(kind as u8), kind, "{kind}");
    }
    assert_eq!(AgentKind::from_repr(0), AgentKind::GenericMcp);
    assert_eq!(AgentKind::from_repr(200), AgentKind::GenericMcp);
    assert!(AgentKind::ALL.contains(&AgentKind::Muse));
}

/// Review of #27: an unknown agent name in persisted state fails to decode (fail closed) rather
/// than being remapped, which would break Merkle digests and rewrite the node on the next save.
#[test]
fn unknown_agent_names_are_rejected() {
    let known: AgentKind = serde_json::from_str("\"Muse\"").expect("known variant");
    assert_eq!(known, AgentKind::Muse);
    assert!(serde_json::from_str::<AgentKind>("\"SomeFutureAgent\"").is_err());
}
