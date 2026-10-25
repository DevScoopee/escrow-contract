#![cfg(test)]
//! Narrower administrative roles (issue #761): `set_admin_role` delegates
//! the Upgrader, Operator, FeeManager and Resolver roles, each held by the
//! root admin until assigned.

use crate::test::setup_funded_escrow;
use crate::{AdminRole, AdminRoleChangedEvent, Error, DISPUTE_TIMEOUT_SECONDS};
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, vec, Address, BytesN, Env, FromVal, Symbol, TryIntoVal};

fn env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env
}

#[test]
fn operator_can_pause_but_cannot_upgrade_or_move_funds() {
    let env = env();
    let (_, _, _, admin, _, _, escrow) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    let operator = Address::generate(&env);
    escrow.set_admin_role(&admin, &AdminRole::Operator, &operator);

    assert_eq!(escrow.try_admin_pause_escrow(&operator), Ok(Ok(())));
    assert_eq!(
        escrow.try_upgrade(&operator, &BytesN::from_array(&env, &[0; 32])),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(
        escrow.try_admin_override_release(&operator, &0),
        Err(Ok(Error::Unauthorized))
    );
    // The root admin no longer holds the delegated role.
    assert_eq!(
        escrow.try_admin_resume_escrow(&admin),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(escrow.try_admin_resume_escrow(&operator), Ok(Ok(())));
}

#[test]
fn upgrader_cannot_pause() {
    let env = env();
    let (_, _, _, admin, _, _, escrow) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    let upgrader = Address::generate(&env);
    escrow.set_admin_role(&admin, &AdminRole::Upgrader, &upgrader);

    assert_eq!(
        escrow.try_admin_pause_escrow(&upgrader),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn fee_manager_sets_the_tax_recipient_and_resolver_overrides() {
    let env = env();
    let (client, freelancer, _, admin, _, _, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    let fees = Address::generate(&env);
    let resolver = Address::generate(&env);
    escrow.set_admin_role(&admin, &AdminRole::FeeManager, &fees);
    escrow.set_admin_role(&admin, &AdminRole::Resolver, &resolver);

    let recipient = Address::generate(&env);
    assert_eq!(
        escrow.try_set_tax_recipient(&resolver, &recipient),
        Err(Ok(Error::Unauthorized))
    );
    escrow.set_tax_recipient(&fees, &recipient);

    escrow.cancel_escrow(&client);
    escrow.cancel_escrow(&freelancer);
    assert_eq!(
        escrow.try_admin_override_cancel_refund(&fees),
        Err(Ok(Error::Unauthorized))
    );
    escrow.admin_override_cancel_refund(&resolver);
}

fn last_role_change(env: &Env) -> AdminRoleChangedEvent {
    let events = crate::all_event_tuples(env);
    let (_, topics, data) = events.last().expect("set_admin_role emits an event");
    let topic: Symbol = topics.get(0).unwrap().try_into_val(env).unwrap();
    assert_eq!(topic, symbol_short!("rolechg"));
    AdminRoleChangedEvent::from_val(env, data)
}

#[test]
fn assigning_the_root_admin_restores_the_default() {
    let env = env();
    let (_, _, _, admin, _, _, escrow) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    let operator = Address::generate(&env);

    escrow.set_admin_role(&admin, &AdminRole::Operator, &operator);
    assert_eq!(
        last_role_change(&env),
        AdminRoleChangedEvent {
            admin: admin.clone(),
            role: AdminRole::Operator,
            previous: admin.clone(),
            new_address: operator.clone(),
        }
    );
    assert_eq!(escrow.get_admin_role(&AdminRole::Operator), operator);

    escrow.set_admin_role(&admin, &AdminRole::Operator, &admin);
    assert_eq!(
        last_role_change(&env),
        AdminRoleChangedEvent {
            admin: admin.clone(),
            role: AdminRole::Operator,
            previous: operator,
            new_address: admin.clone(),
        }
    );
    assert_eq!(escrow.get_admin_role(&AdminRole::Operator), admin);
    assert_eq!(escrow.try_admin_pause_escrow(&admin), Ok(Ok(())));
}

#[test]
fn only_the_root_admin_assigns_roles() {
    let env = env();
    let (_, _, _, admin, _, _, escrow) = setup_funded_escrow(&env, vec![&env, 1_000_i128]);
    let operator = Address::generate(&env);
    escrow.set_admin_role(&admin, &AdminRole::Operator, &operator);

    assert_eq!(
        escrow.try_set_admin_role(&operator, &AdminRole::Resolver, &operator),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(escrow.get_admin_role(&AdminRole::Resolver), admin);
}

#[test]
fn cancel_override_waits_for_an_open_dispute_to_time_out() {
    let env = env();
    let (client, freelancer, _, admin, _, _, escrow) =
        setup_funded_escrow(&env, vec![&env, 1_000_i128, 2_000]);
    escrow.mark_delivered(&freelancer, &0);
    escrow.raise_dispute(&client, &0, &BytesN::from_array(&env, &[5; 32]));
    escrow.cancel_escrow(&client);
    escrow.cancel_escrow(&freelancer);

    assert_eq!(
        escrow.try_admin_override_cancel_refund(&admin),
        Err(Ok(Error::DisputeTimeoutNotReached))
    );

    env.ledger()
        .with_mut(|l| l.timestamp += DISPUTE_TIMEOUT_SECONDS);
    escrow.admin_override_cancel_refund(&admin);
}
