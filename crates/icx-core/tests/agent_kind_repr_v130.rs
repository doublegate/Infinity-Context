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

/// Review of #27: a future agent name in persisted state decodes as `GenericMcp` instead of failing.
#[test]
fn unknown_agent_names_deserialize_as_generic_mcp() {
    let known: AgentKind = serde_json::from_str("\"Muse\"").expect("known variant");
    assert_eq!(known, AgentKind::Muse);
    let future: AgentKind = serde_json::from_str("\"SomeFutureAgent\"").expect("unknown variant");
    assert_eq!(future, AgentKind::GenericMcp);
}
