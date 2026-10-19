#![cfg(test)]
//! `is_escrow_interest_yield_locked` reports whether the interest/yield share
//! configuration is frozen, returning `NotInitialized` until a configuration
//! has been written.

use crate::testutils::EscrowFixture;
use crate::Error;
use soroban_sdk::Env;

#[test]
fn uninitialized_contract_returns_not_initialized() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env).uninitialized().build();

    assert_eq!(
        fx.escrow.try_is_escrow_interest_yield_locked(),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn missing_configuration_returns_not_initialized() {
    let env = Env::default();
    let fx = EscrowFixture::new(&env).funded().build();

    assert_eq!(
        fx.escrow.try_is_escrow_interest_yield_locked(),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn reports_the_lock_flag_through_lock_and_unlock() {
    let env = Env::default();
    env.mock_all_auths();
    let fx = EscrowFixture::new(&env).funded().build();
    let (escrow, admin) = (&fx.escrow, &fx.admin);

    escrow.set_escrow_interest_yield(admin, &5_000, &5_000);
    assert!(!escrow.is_escrow_interest_yield_locked());

    escrow.lock_escrow_interest_yield(admin);
    assert!(escrow.is_escrow_interest_yield_locked());

    escrow.unlock_escrow_interest_yield(admin);
    assert!(!escrow.is_escrow_interest_yield_locked());
}

#[test]
fn read_mutates_no_ledger_entry_and_publishes_no_event() {
    let env = Env::default();
    env.mock_all_auths();
    let fx = EscrowFixture::new(&env).funded().build();
    fx.escrow
        .set_escrow_interest_yield(&fx.admin, &5_000, &5_000);
    fx.escrow.lock_escrow_interest_yield(&fx.admin);

    let before = env.to_ledger_snapshot();
    assert!(fx.escrow.is_escrow_interest_yield_locked());
    assert_eq!(
        crate::all_event_tuples(&env).len(),
        0,
        "a read publishes no event"
    );
    assert!(fx.escrow.is_escrow_interest_yield_locked());
    let after = env.to_ledger_snapshot();

    assert_eq!(before, after, "a read mutates no ledger entry");
}
