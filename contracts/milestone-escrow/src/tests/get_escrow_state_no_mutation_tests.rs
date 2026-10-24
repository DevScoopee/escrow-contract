#![cfg(test)]
//! `get_escrow_state` bundles everything a UI needs to render an escrow (job,
//! admin, both pause flags, cancellation lock and approvals, per-milestone
//! extensions and deadlines, fee allocation, version) into one read.
//!
//! Like the other `*_no_mutation_tests`, the read-only guarantee is checked
//! by comparing whole-ledger snapshots (`Env::to_ledger_snapshot`, which
//! includes every entry and its TTL) taken before and after the call, and by
//! asserting the call publishes no event.

use crate::test::setup_funded_escrow;
use crate::{Error, MilestoneEscrow, MilestoneEscrowClient};
use soroban_sdk::{vec, Env};

fn assert_get_escrow_state_is_read_only(env: &Env, escrow: &MilestoneEscrowClient<'_>) {
    let before = env.to_ledger_snapshot();
    escrow.get_escrow_state();
    assert_eq!(
        crate::all_event_tuples(env).len(),
        0,
        "get_escrow_state publishes no event"
    );
    escrow.get_escrow_state();
    let after = env.to_ledger_snapshot();
    assert_eq!(before, after, "get_escrow_state mutated the ledger");
}

#[test]
fn get_escrow_state_does_not_mutate_ledger() {
    let env = Env::default();
    env.mock_all_auths();
    let (_, _, _, _, _, _, escrow) = setup_funded_escrow(&env, vec![&env, 1_000_i128, 2_000]);

    assert_get_escrow_state_is_read_only(&env, &escrow);
}

#[test]
fn get_escrow_state_does_not_mutate_ledger_with_extensions_and_cancel_approval() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, freelancer, _, _, _, _, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128, 2_000]);
    escrow.mark_delivered(&freelancer, &0);
    escrow.extend_milestone_deadline(&client, &0, &120);
    escrow.cancel_escrow(&client);

    assert_get_escrow_state_is_read_only(&env, &escrow);
}

#[test]
fn get_escrow_state_bundles_the_full_render_state() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, freelancer, arbiter, admin, token, _, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128, 2_000]);
    escrow.mark_delivered(&freelancer, &0);
    escrow.extend_milestone_deadline(&client, &0, &120);
    escrow.cancel_escrow(&client);

    let state = escrow.get_escrow_state();

    // Same job as get_job.
    let job = escrow.get_job();
    assert_eq!(state.job.client, client);
    assert_eq!(state.job.freelancer, freelancer);
    assert_eq!(state.job.arbiter, arbiter);
    assert_eq!(state.job.token, token);
    assert_eq!(state.job.milestones.len(), job.milestones.len());
    assert_eq!(state.job.funded, job.funded);

    assert_eq!(state.admin, admin);
    assert!(!state.paused.emergency);
    assert!(!state.paused.admin);
    // The client approved cancellation (bit 1); the escrow is not locked yet.
    assert_eq!(state.cancel.approval_mask, 1);
    assert!(!state.cancel.locked);

    assert_eq!(state.milestone_timing.len(), 2);
    let delivered = state.milestone_timing.get(0).unwrap();
    assert_eq!(delivered.extension_seconds, 120);
    let m0 = job.milestones.get(0).unwrap();
    assert_eq!(
        delivered.deadline,
        Some(m0.delivered_at + job.auto_release_seconds + 120)
    );
    let pending = state.milestone_timing.get(1).unwrap();
    assert_eq!(pending.extension_seconds, 0);
    assert_eq!(pending.deadline, None, "no deadline before delivery");

    assert_eq!(state.version, escrow.version());
    assert_eq!(
        state.fee_allocation_set,
        escrow.try_get_platform_fee_allocation().is_ok()
    );
}

#[test]
fn get_escrow_state_reports_the_emergency_pause() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, freelancer, _, _, _, _, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    escrow.emergency_pause(&client, &freelancer);

    let state = escrow.get_escrow_state();
    assert!(state.paused.emergency);
    assert_eq!(state.paused.emergency, escrow.is_emergency_paused());
}

#[test]
fn get_escrow_state_on_uninitialized_contract_returns_not_initialized_without_mutation() {
    let env = Env::default();
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(&env, &contract_id);

    let before = env.to_ledger_snapshot();
    assert!(matches!(
        escrow.try_get_escrow_state(),
        Err(Ok(Error::NotInitialized))
    ));
    let after = env.to_ledger_snapshot();
    assert_eq!(
        before, after,
        "a rejected get_escrow_state mutated the ledger"
    );
}
