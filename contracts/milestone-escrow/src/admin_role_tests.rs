#![cfg(test)]

use super::*;
use soroban_sdk::{Address, BytesN, Env, Vec};

fn initialized_escrow(env: &Env) -> (MilestoneEscrowClient<'_>, Address) {
    env.mock_all_auths();

    let admin = Address::generate(env);
    let client = Address::generate(env);
    let freelancer = Address::generate(env);
    let arbiter = Address::generate(env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let contract_id = env.register(MilestoneEscrow, ());
    let escrow = MilestoneEscrowClient::new(env, &contract_id);
    escrow.initialize(
        &admin,
        &client,
        &freelancer,
        &arbiter,
        &token,
        &604_800,
        &Vec::from_array(env, [1_000_i128]),
    );
    (escrow, admin)
}

#[test]
fn operator_can_pause_but_cannot_upgrade_or_move_funds() {
    let env = Env::default();
    let (escrow, admin) = initialized_escrow(&env);
    let operator = Address::generate(&env);
    escrow.set_admin_role(&admin, &AdminRole::Operator, &operator);

    assert_eq!(escrow.try_admin_pause_escrow(&operator), Ok(Ok(())));

    let wasm_hash = BytesN::from_array(&env, &[0; 32]);
    assert_eq!(
        escrow.try_upgrade(&operator, &wasm_hash),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(
        escrow.try_admin_override_release(&operator, &0),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn upgrader_cannot_pause() {
    let env = Env::default();
    let (escrow, admin) = initialized_escrow(&env);
    let upgrader = Address::generate(&env);
    escrow.set_admin_role(&admin, &AdminRole::Upgrader, &upgrader);

    assert_eq!(
        escrow.try_admin_pause_escrow(&upgrader),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn assigning_current_admin_restores_default_role() {
    let env = Env::default();
    let (escrow, admin) = initialized_escrow(&env);
    let operator = Address::generate(&env);
    escrow.set_admin_role(&admin, &AdminRole::Operator, &operator);
    escrow.set_admin_role(&admin, &AdminRole::Operator, &admin);

    assert_eq!(escrow.try_admin_pause_escrow(&admin), Ok(Ok(())));
}