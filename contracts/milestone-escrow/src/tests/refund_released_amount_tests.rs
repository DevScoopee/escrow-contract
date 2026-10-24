#![cfg(test)]
//! Tests that refund paths leave `released_amount` unchanged (issue #688).
//!
//! `released_amount` represents how much of the milestone has been paid to
//! the freelancer.  Before this fix, the four refund paths below incorrectly
//! set `released_amount = milestone.amount`, making refunded funds appear as
//! freelancer payouts in any consumer that sums `released_amount`.
//!
//! Each path is tested twice:
//!   * untouched milestone — expect `released_amount == 0` after refund.
//!   * 30%-partially-released milestone — expect `released_amount == 300`
//!     (unchanged from before the refund) after the remaining 70% is refunded.
//!
//! Paths covered:
//!   1. `admin_override_refund`
//!   2. `admin_override_cancel_refund`
//!   3. `admin_override_tax_refund`
//!   4. `multisig_admin_override_refund`

use super::*;
use crate::{DataKey, MilestoneStatus, TaxWithholdingRecord};
use soroban_sdk::{vec, Address, Env};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Read `released_amount` and `status` for milestone 0 directly from storage.
fn read_milestone_0(env: &Env, contract_id: &Address) -> (i128, MilestoneStatus) {
    env.as_contract(contract_id, || {
        let m: Milestone = env
            .storage()
            .persistent()
            .get(&DataKey::Milestone(0u32))
            .unwrap();
        (m.released_amount, m.status)
    })
}

/// Partially release `amount_released` of milestone 0 via `approve_partial`,
/// bringing `released_amount` to `amount_released` while status stays
/// `PartiallyReleased`.
fn partial_release(
    _env: &Env,
    escrow: &MilestoneEscrowClient,
    client_addr: &Address,
    freelancer_addr: &Address,
    amount_released: i128,
) {
    escrow.mark_delivered(freelancer_addr, &0u32);
    escrow.approve_partial(client_addr, &0u32, &amount_released);
}

// ── 1. admin_override_refund ──────────────────────────────────────────────────

#[test]
fn admin_override_refund_untouched_milestone_leaves_released_amount_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, admin_addr, _, contract_id, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    // Raise a dispute so the override precondition is satisfied.
    escrow.mark_delivered(&freelancer_addr, &0u32);
    escrow.raise_dispute(&client_addr, &0u32, &dispute_reason_hash(&env));

    escrow.admin_override_refund(&admin_addr, &0u32);

    let (released_amount, status) = read_milestone_0(&env, &contract_id);
    assert_eq!(status, MilestoneStatus::Refunded);
    assert_eq!(
        released_amount, 0,
        "refunding an untouched milestone must leave released_amount == 0"
    );
}

#[test]
fn admin_override_refund_partial_release_preserves_released_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, admin_addr, _, contract_id, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    // Release 300 (30%) first, then dispute and override-refund the rest.
    partial_release(&env, &escrow, &client_addr, &freelancer_addr, 300);
    escrow.raise_dispute(&client_addr, &0u32, &dispute_reason_hash(&env));

    escrow.admin_override_refund(&admin_addr, &0u32);

    let (released_amount, status) = read_milestone_0(&env, &contract_id);
    assert_eq!(status, MilestoneStatus::Refunded);
    assert_eq!(
        released_amount, 300,
        "refunding after 30% release must keep released_amount == 300"
    );
}

// ── 2. admin_override_cancel_refund ──────────────────────────────────────────

#[test]
fn admin_override_cancel_refund_untouched_milestone_leaves_released_amount_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, admin_addr, _, contract_id, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    // Fire the two-party cancel lock.
    escrow.cancel_escrow(&client_addr);
    escrow.cancel_escrow(&freelancer_addr);

    escrow.admin_override_cancel_refund(&admin_addr);

    let (released_amount, status) = read_milestone_0(&env, &contract_id);
    assert_eq!(status, MilestoneStatus::Refunded);
    assert_eq!(
        released_amount, 0,
        "cancel-refunding an untouched milestone must leave released_amount == 0"
    );
    let _ = freelancer_addr; // suppress unused-variable lint
}

#[test]
fn admin_override_cancel_refund_partial_release_preserves_released_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, admin_addr, _, contract_id, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    // Release 300 first, then cancel-lock and refund the rest.
    partial_release(&env, &escrow, &client_addr, &freelancer_addr, 300);
    // Return to Pending so cancel_escrow can be raised (mark_delivered was
    // already called by partial_release; at 300 of 1000 it's PartiallyReleased).
    // Fire cancel directly — it works on any non-terminal status.
    escrow.cancel_escrow(&client_addr);
    escrow.cancel_escrow(&freelancer_addr);

    escrow.admin_override_cancel_refund(&admin_addr);

    let (released_amount, status) = read_milestone_0(&env, &contract_id);
    assert_eq!(status, MilestoneStatus::Refunded);
    assert_eq!(
        released_amount, 300,
        "cancel-refunding after 30% release must keep released_amount == 300"
    );
}

// ── 3. admin_override_tax_refund ─────────────────────────────────────────────

fn seed_tax_lock(env: &Env, contract_id: &Address, gross: i128, tax: i128, net: i128) {
    env.as_contract(contract_id, || {
        env.storage().persistent().set(
            &DataKey::TaxWithholdingLock(0u32),
            &TaxWithholdingRecord {
                gross_amount: gross,
                tax_amount: tax,
                net_amount: net,
                tax_rate_bps: 1_000,
            },
        );
    });
}

#[test]
fn admin_override_tax_refund_untouched_milestone_leaves_released_amount_zero() {
    let env = Env::default();
    env.mock_all_auths();

    let (_, _, _, admin_addr, _, contract_id, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    // Seed the tax withholding lock manually (simulates tax_withholding_deductions).
    seed_tax_lock(&env, &contract_id, 1_000, 100, 900);

    // Set milestone to Disputed so admin_override_tax_refund's precondition passes.
    env.as_contract(&contract_id, || {
        let mut m: Milestone = env
            .storage()
            .persistent()
            .get(&DataKey::Milestone(0u32))
            .unwrap();
        m.status = MilestoneStatus::Disputed;
        env.storage()
            .persistent()
            .set(&DataKey::Milestone(0u32), &m);
    });

    escrow.admin_override_tax_refund(&admin_addr, &0u32);

    let (released_amount, status) = read_milestone_0(&env, &contract_id);
    assert_eq!(status, MilestoneStatus::Refunded);
    assert_eq!(
        released_amount, 0,
        "tax-refunding an untouched milestone must leave released_amount == 0"
    );
}

#[test]
fn admin_override_tax_refund_partial_release_preserves_released_amount() {
    let env = Env::default();
    env.mock_all_auths();

    let (client_addr, freelancer_addr, _, admin_addr, _, contract_id, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);

    // Release 300 first.
    partial_release(&env, &escrow, &client_addr, &freelancer_addr, 300);

    // Seed the tax lock for the remaining 700.
    seed_tax_lock(&env, &contract_id, 700, 70, 630);

    // Set milestone to Disputed so the precondition passes.
    env.as_contract(&contract_id, || {
        let mut m: Milestone = env
            .storage()
            .persistent()
            .get(&DataKey::Milestone(0u32))
            .unwrap();
        m.status = MilestoneStatus::Disputed;
        env.storage()
            .persistent()
            .set(&DataKey::Milestone(0u32), &m);
    });

    escrow.admin_override_tax_refund(&admin_addr, &0u32);

    let (released_amount, status) = read_milestone_0(&env, &contract_id);
    assert_eq!(status, MilestoneStatus::Refunded);
    assert_eq!(
        released_amount, 300,
        "tax-refunding after 30% release must keep released_amount == 300"
    );
}

// ── 4. multisig_admin_override_refund ────────────────────────────────────────

fn setup_multisig_funded(
    env: &Env,
) -> (
    Address,
    Address,
    Address,
    Address,
    Address,
    MilestoneEscrowClient<'_>,
) {
    env.mock_all_auths();
    let admin = Address::generate(env);
    let client = Address::generate(env);
    let freelancer = Address::generate(env);
    let arbiter = Address::generate(env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let token_admin = soroban_sdk::token::StellarAssetClient::new(env, &token);
    token_admin.mint(&client, &1_000);
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(env, &contract_id);
    let amounts = vec![env, 1_000_i128];
    escrow.initialize(
        &admin,
        &client,
        &freelancer,
        &arbiter,
        &token,
        &604_800u64,
        &amounts,
        &crate::test_spec_hashes(env, &amounts),
    );
    escrow.accept_job(&freelancer);
    escrow.accept_arbiter(&arbiter);
    escrow.fund(&client);
    // The multisig override only needs the multisig lock.
    escrow.multisig_lock(&admin);
    (contract_id, admin, client, freelancer, arbiter, escrow)
}

#[test]
fn multisig_admin_override_refund_untouched_milestone_leaves_released_amount_zero() {
    let env = Env::default();
    let (contract_id, admin, _, _, _, escrow) = setup_multisig_funded(&env);

    escrow.multisig_admin_override_refund(&admin, &0u32);

    let (released_amount, status) = read_milestone_0(&env, &contract_id);
    assert_eq!(status, MilestoneStatus::Refunded);
    assert_eq!(
        released_amount, 0,
        "multisig-refunding an untouched milestone must leave released_amount == 0"
    );
}

#[test]
fn multisig_admin_override_refund_partial_release_preserves_released_amount() {
    let env = Env::default();
    let (contract_id, admin, client, freelancer, _, escrow) = setup_multisig_funded(&env);

    // Release 300 first (multisig_lock is active from setup_multisig_funded,
    // so we clear it temporarily to allow approve_partial, then re-set it).
    env.as_contract(&contract_id, || {
        env.storage().instance().remove(&DataKey::MultisigLocked);
    });

    escrow.mark_delivered(&freelancer, &0u32);
    escrow.approve_partial(&client, &0u32, &300);

    // Re-arm the multisig lock so multisig_admin_override_refund's
    // precondition is satisfied.
    env.as_contract(&contract_id, || {
        env.storage()
            .instance()
            .set(&DataKey::MultisigLocked, &true);
    });

    escrow.multisig_admin_override_refund(&admin, &0u32);

    let (released_amount, status) = read_milestone_0(&env, &contract_id);
    assert_eq!(status, MilestoneStatus::Refunded);
    assert_eq!(
        released_amount, 300,
        "multisig-refunding after 30% release must keep released_amount == 300"
    );
}
