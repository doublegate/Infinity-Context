//! v1.1.0 MCP resource bounds (ICX-AUD-014).

use icx_adapters::mcp::{MAX_FOLD_DEPTH, McpPagerServer};
use icx_adapters::mmu::InfinityContextMmu;
use serde_json::json;

#[test]
fn nested_fold_branches_are_bounded() {
    let mut mmu = InfinityContextMmu::default();
    for i in 0..MAX_FOLD_DEPTH {
        let out = McpPagerServer::execute_icx_op(
            &json!({"op": "fold", "q": format!("begin:b{i}")}),
            &mut mmu,
        );
        assert!(out.contains("BRANCH_OPENED"), "level {i}: {out}");
    }
    let refused =
        McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "begin:one-too-many"}), &mut mmu);
    assert!(refused.contains("ERR"), "{refused}");
    assert_eq!(mmu.dag.active_branch_depth(), MAX_FOLD_DEPTH);
    // Folding still works and frees a level.
    let folded = McpPagerServer::execute_icx_op(&json!({"op": "fold", "q": "done"}), &mut mmu);
    assert!(folded.contains("BRANCH_FOLDED"), "{folded}");
}
