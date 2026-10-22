#![cfg(test)]
//! Rotating the multisig signer set and threshold through the proposal /
//! approval flow: `propose_multisig_config`, `execute_multisig_config`,
//! `cancel_multisig_config_proposal`.
//!
//! Approvals are a bitmap over signer indices, so after a rotation bit `i`
//! would name a different signer. Each rotation therefore starts a new
//! approval generation: approvals collected under the old signer set do not
//! count for anything afterwards.

use crate::test::setup_funded_escrow;
use crate::{Error, MilestoneEscrowClient, MultiSigConfigChangedEvent, PendingMultiSigConfig};
use soroban_sdk::testutils::{Address as _, EnvTestConfig};
use soroban_sdk::{symbol_short, vec, Address, Env, Symbol, TryFromVal, Vec};

fn test_env() -> Env {
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    env
}

struct Setup<'a> {
    admin: Address,
    old: Vec<Address>,
    new: Vec<Address>,
    escrow: MilestoneEscrowClient<'a>,
}

/// Funded escrow with a 2-of-3 multisig regime, plus a fresh 2-of-2 set to
/// rotate to.
fn setup(env: &Env) -> Setup<'_> {
    let (_, _, _, admin, _, _, escrow) = setup_funded_escrow(env, vec![env, 1_000_i128]);
    let old = vec![
        env,
        Address::generate(env),
        Address::generate(env),
        Address::generate(env),
    ];
    escrow.multisig_approval_init(&admin, &old, &2);
    let new = vec![env, Address::generate(env), Address::generate(env)];
    Setup {
        admin,
        old,
        new,
        escrow,
    }
}

fn config_events(env: &Env) -> std::vec::Vec<MultiSigConfigChangedEvent> {
    let topic = symbol_short!("msigcfg");
    crate::all_event_tuples(env)
        .into_iter()
        .filter(|(_, topics, _)| {
            topics.len() == 1
                && Symbol::try_from_val(env, &topics.get(0).unwrap()).ok() == Some(topic.clone())
        })
        .map(|(_, _, data)| MultiSigConfigChangedEvent::try_from_val(env, &data).unwrap())
        .collect()
}

#[test]
fn rotation_with_threshold_approvals_succeeds() {
    let env = test_env();
    let s = setup(&env);

    s.escrow.propose_multisig_config(&s.admin, &s.new, &2, &7);
    assert_eq!(
        s.escrow.get_pending_multisig_config(),
        Some(PendingMultiSigConfig {
            signers: s.new.clone(),
            threshold: 2,
            proposal_id: 7,
        })
    );
    s.escrow.multisig_approve(&s.old.get(0).unwrap(), &7);
    s.escrow.multisig_approve(&s.old.get(2).unwrap(), &7);

    s.escrow.execute_multisig_config();
    assert_eq!(
        config_events(&env),
        std::vec![MultiSigConfigChangedEvent {
            old_signers: s.old.clone(),
            old_threshold: 2,
            new_signers: s.new.clone(),
            new_threshold: 2,
            proposal_id: 7,
            generation: 1,
        }]
    );
    assert_eq!(s.escrow.get_pending_multisig_config(), None);

    // The new set is in force: its members can approve, the old ones cannot.
    s.escrow.multisig_approve(&s.new.get(0).unwrap(), &8);
    let state = s.escrow.multisig_approve(&s.new.get(1).unwrap(), &8);
    assert!(state.approved);
    assert_eq!(state.threshold, 2);
    assert_eq!(
        s.escrow.try_multisig_approve(&s.old.get(0).unwrap(), &9),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn rotation_without_threshold_approvals_fails() {
    let env = test_env();
    let s = setup(&env);

    s.escrow.propose_multisig_config(&s.admin, &s.new, &2, &7);
    s.escrow.multisig_approve(&s.old.get(0).unwrap(), &7);
    assert_eq!(
        s.escrow.try_execute_multisig_config(),
        Err(Ok(Error::MultiSigThresholdNotMet))
    );

    // Still the old regime, and the proposal is still pending.
    assert_eq!(
        s.escrow.try_multisig_approve(&s.new.get(0).unwrap(), &8),
        Err(Ok(Error::Unauthorized))
    );
    assert!(s.escrow.get_pending_multisig_config().is_some());
    assert!(config_events(&env).is_empty());

    // Approvals from addresses outside the current set do not help.
    assert_eq!(
        s.escrow.try_multisig_approve(&s.new.get(1).unwrap(), &7),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(
        s.escrow.try_execute_multisig_config(),
        Err(Ok(Error::MultiSigThresholdNotMet))
    );
}

#[test]
fn old_approvals_do_not_carry_over() {
    let env = test_env();
    let s = setup(&env);

    // Proposal 3 reaches the threshold under the old set...
    s.escrow.multisig_approve(&s.old.get(0).unwrap(), &3);
    s.escrow.multisig_approve(&s.old.get(1).unwrap(), &3);
    assert!(s.escrow.is_multisig_approved(&3).approved);
    // ...and an admin transfer is half-way through approval.
    let next_admin = Address::generate(&env);
    s.escrow.propose_admin_transfer(&s.admin, &next_admin, &4);
    s.escrow.multisig_approve(&s.old.get(0).unwrap(), &4);
    s.escrow.multisig_approve(&s.old.get(1).unwrap(), &4);

    s.escrow.propose_multisig_config(&s.admin, &s.new, &2, &7);
    s.escrow.multisig_approve(&s.old.get(0).unwrap(), &7);
    s.escrow.multisig_approve(&s.old.get(1).unwrap(), &7);
    s.escrow.execute_multisig_config();

    // Nothing approved under the old signer set counts any more.
    for proposal in [3u32, 4, 7] {
        let state = s.escrow.is_multisig_approved(&proposal);
        assert_eq!(state.approvals, 0, "proposal {proposal}");
        assert!(!state.approved);
    }
    assert_eq!(
        s.escrow.try_execute_admin_transfer(),
        Err(Ok(Error::MultiSigThresholdNotMet))
    );

    // The new signers have to approve it themselves.
    s.escrow.multisig_approve(&s.new.get(0).unwrap(), &4);
    s.escrow.multisig_approve(&s.new.get(1).unwrap(), &4);
    s.escrow.execute_admin_transfer();
}

#[test]
fn proposal_is_validated_and_admin_only() {
    let env = test_env();
    let s = setup(&env);
    let a = Address::generate(&env);

    assert_eq!(
        s.escrow
            .try_propose_multisig_config(&Address::generate(&env), &s.new, &2, &7),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(
        s.escrow
            .try_propose_multisig_config(&s.admin, &Vec::new(&env), &1, &7),
        Err(Ok(Error::MultiSigNoSigners))
    );
    assert_eq!(
        s.escrow
            .try_propose_multisig_config(&s.admin, &s.new, &3, &7),
        Err(Ok(Error::MultiSigInvalidThreshold))
    );
    assert_eq!(
        s.escrow
            .try_propose_multisig_config(&s.admin, &vec![&env, a.clone(), a], &1, &7),
        Err(Ok(Error::MultiSigDuplicateSigner))
    );
    assert_eq!(s.escrow.get_pending_multisig_config(), None);
}

#[test]
fn proposal_needs_an_existing_regime_and_a_fresh_proposal_id() {
    let env = test_env();
    let (_, _, _, admin, _, _, escrow) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    let new = vec![&env, Address::generate(&env)];
    assert_eq!(
        escrow.try_propose_multisig_config(&admin, &new, &1, &7),
        Err(Ok(Error::NotInitialized))
    );

    let s = setup(&env);
    // An id that already carries approvals would start out pre-approved.
    s.escrow.multisig_approve(&s.old.get(0).unwrap(), &5);
    assert_eq!(
        s.escrow
            .try_propose_multisig_config(&s.admin, &s.new, &2, &5),
        Err(Ok(Error::ProposalIdInUse))
    );
    // So would the id of a pending admin transfer.
    s.escrow
        .propose_admin_transfer(&s.admin, &Address::generate(&env), &6);
    assert_eq!(
        s.escrow
            .try_propose_multisig_config(&s.admin, &s.new, &2, &6),
        Err(Ok(Error::ProposalIdInUse))
    );
}

#[test]
fn one_pending_proposal_at_a_time_and_cancel_clears_it() {
    let env = test_env();
    let s = setup(&env);

    assert_eq!(
        s.escrow.try_execute_multisig_config(),
        Err(Ok(Error::NoPendingMultiSigConfig))
    );
    s.escrow.propose_multisig_config(&s.admin, &s.new, &2, &7);
    assert_eq!(
        s.escrow
            .try_propose_multisig_config(&s.admin, &s.new, &1, &8),
        Err(Ok(Error::MultiSigConfigPending))
    );

    assert_eq!(
        s.escrow
            .try_cancel_multisig_config_proposal(&Address::generate(&env)),
        Err(Ok(Error::Unauthorized))
    );
    s.escrow.cancel_multisig_config_proposal(&s.admin);
    assert_eq!(s.escrow.get_pending_multisig_config(), None);
    assert_eq!(
        s.escrow.try_cancel_multisig_config_proposal(&s.admin),
        Err(Ok(Error::NoPendingMultiSigConfig))
    );
    s.escrow.propose_multisig_config(&s.admin, &s.new, &1, &8);
}
