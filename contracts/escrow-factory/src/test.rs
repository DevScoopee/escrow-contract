#![cfg(test)]

use soroban_sdk::{
    symbol_short, testutils::Address as _, token, vec, Address, BytesN, Env, FromVal, TryIntoVal,
};

use crate::{EscrowCreatedEvent, EscrowFactory, EscrowFactoryClient, Error};

// Import the compiled milestone-escrow WASM so the test env can deploy real
// escrow instances through the factory.
mod escrow {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/milestone_escrow.wasm"
    );
}

fn zero_hashes(env: &Env, n: u32) -> soroban_sdk::Vec<BytesN<32>> {
    let mut v = soroban_sdk::Vec::new(env);
    for _ in 0..n {
        v.push_back(BytesN::from_array(env, &[0u8; 32]));
    }
    v
}

fn mint_token(env: &Env, recipient: &Address, amount: i128) -> Address {
    let admin = Address::generate(env);
    let tok = env
        .register_stellar_asset_contract_v2(admin)
        .address();
    token::StellarAssetClient::new(env, &tok).mint(recipient, &amount);
    tok
}

// Returns (env, factory_id, admin, whitelisted_token, wasm_hash).
// Tests construct their own client from factory_id to avoid lifetime issues.
fn setup() -> (Env, Address, Address, Address, BytesN<32>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let wasm_hash = env.deployer().upload_contract_wasm(escrow::WASM);

    let factory_id = env.register(EscrowFactory, ());
    let factory = EscrowFactoryClient::new(&env, &factory_id);
    factory.initialize(&admin, &wasm_hash);

    let token = env
        .register_stellar_asset_contract_v2(Address::generate(&env))
        .address();
    factory.allow_token(&admin, &token);

    (env, factory_id, admin, token, wasm_hash)
}

#[test]
fn initialize_and_getters() {
    let (env, fid, admin, token, wasm_hash) = setup();
    let factory = EscrowFactoryClient::new(&env, &fid);
    assert_eq!(factory.get_admin(), admin);
    assert_eq!(factory.get_wasm_hash(), wasm_hash);
    let tokens = factory.get_allowed_tokens();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.get(0).unwrap(), token);
}

#[test]
fn double_initialize_rejected() {
    let (env, fid, admin, _token, wasm_hash) = setup();
    let factory = EscrowFactoryClient::new(&env, &fid);
    assert_eq!(
        factory.try_initialize(&admin, &wasm_hash),
        Err(Ok(Error::AlreadyInitialized))
    );
}

#[test]
fn allow_token_idempotent() {
    let (env, fid, admin, token, _wasm_hash) = setup();
    let factory = EscrowFactoryClient::new(&env, &fid);
    factory.allow_token(&admin, &token);
    assert_eq!(factory.get_allowed_tokens().len(), 1);
}

#[test]
fn create_escrow_unlisted_token_rejected() {
    let (env, fid, _admin, _token, _wasm_hash) = setup();
    let factory = EscrowFactoryClient::new(&env, &fid);
    assert_eq!(
        factory.try_create_escrow(
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env), // not whitelisted
            &604800u64,
            &vec![&env, 1_000_i128],
            &zero_hashes(&env, 1),
            &BytesN::from_array(&env, &[1u8; 32]),
        ),
        Err(Ok(Error::TokenNotAllowed))
    );
}

#[test]
fn create_escrow_empty_milestones_rejected() {
    let (env, fid, _admin, token, _wasm_hash) = setup();
    let factory = EscrowFactoryClient::new(&env, &fid);
    assert_eq!(
        factory.try_create_escrow(
            &Address::generate(&env),
            &Address::generate(&env),
            &Address::generate(&env),
            &token,
            &604800u64,
            &soroban_sdk::Vec::new(&env),
            &soroban_sdk::Vec::new(&env),
            &BytesN::from_array(&env, &[2u8; 32]),
        ),
        Err(Ok(Error::InvalidAmount))
    );
}

/// Deploy two escrows for the same client; verify they are independent by
/// running fund -> deliver -> approve on both.
#[test]
fn two_escrows_independent_lifecycle() {
    let (env, fid, admin, _default_tok, _wasm_hash) = setup();
    let factory = EscrowFactoryClient::new(&env, &fid);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let arbiter = Address::generate(&env);

    let tok = mint_token(&env, &client, 2_000);
    factory.allow_token(&admin, &tok);

    let amounts = vec![&env, 1_000_i128];
    let hashes = zero_hashes(&env, 1);

    let escrow1 = factory.create_escrow(
        &client, &freelancer, &arbiter, &tok, &604800u64,
        &amounts, &hashes, &BytesN::from_array(&env, &[10u8; 32]),
    );
    let escrow2 = factory.create_escrow(
        &client, &freelancer, &arbiter, &tok, &604800u64,
        &amounts, &hashes, &BytesN::from_array(&env, &[20u8; 32]),
    );

    assert_ne!(escrow1, escrow2);

    let e1 = escrow::Client::new(&env, &escrow1);
    let e2 = escrow::Client::new(&env, &escrow2);

    e1.accept_job(&freelancer);
    e1.accept_arbiter(&arbiter);
    e2.accept_job(&freelancer);
    e2.accept_arbiter(&arbiter);

    e1.fund(&client);
    e1.mark_delivered(&freelancer, &0u32);
    e1.approve_milestone(&client, &0u32);

    e2.fund(&client);
    e2.mark_delivered(&freelancer, &0u32);
    e2.approve_milestone(&client, &0u32);

    assert_eq!(token::Client::new(&env, &tok).balance(&freelancer), 2_000);
}

#[test]
fn created_event_carries_address_and_parties() {
    let (env, fid, admin, _default_tok, _wasm_hash) = setup();
    let factory = EscrowFactoryClient::new(&env, &fid);

    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let arbiter = Address::generate(&env);
    let tok = mint_token(&env, &client, 1_000);
    factory.allow_token(&admin, &tok);

    let escrow = factory.create_escrow(
        &client, &freelancer, &arbiter, &tok, &604800u64,
        &vec![&env, 1_000_i128],
        &zero_hashes(&env, 1),
        &BytesN::from_array(&env, &[3u8; 32]),
    );

    let events = env.events().all();
    let (_, topics, data) = events.last().unwrap();
    let topic: soroban_sdk::Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
    assert_eq!(topic, symbol_short!("created"));

    let event = EscrowCreatedEvent::from_val(&env, &data);
    assert_eq!(event.escrow, escrow);
    assert_eq!(event.client, client);
    assert_eq!(event.freelancer, freelancer);
    assert_eq!(event.arbiter, arbiter);
    assert_eq!(event.token, tok);
}
