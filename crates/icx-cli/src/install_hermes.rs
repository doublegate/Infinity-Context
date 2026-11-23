//! `icx install --agent hermes` (T-65-004): installs the Hermes plugins shipped in
//! `integrations/hermes/` (T-65-002, T-65-009) and prints the one config change Hermes needs,
//! which icx does not make itself: Hermes keeps its settings in YAML, and icx edits only formats
//! it can rewrite without losing comments.

use crate::install::{Opts, Plan};

pub(crate) fn plan(p: &mut Plan, o: &Opts) {
    let _ = o;
    p.notes.push(
        "Hermes: no Hermes plugin is shipped yet in this build; see the deployment guide section 2.5."
            .to_owned(),
    );
}
