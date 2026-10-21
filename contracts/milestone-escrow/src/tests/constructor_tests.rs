#![cfg(test)]

use super::*;
use soroban_sdk::testutils::EnvTestConfig;
use soroban_sdk::{vec, Address, BytesN, Env, Vec};

fn test_env() -> Env {
    Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    })
}

#[test]
fn deploy_with_constructor_args_succeeds_and_asserts_stored_state() {
    let env = test_env();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let client = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let arbiter = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    let amounts: Vec<i128> = vec![&env, 400_i128, 600_i128];
    let spec_hashes: Vec<BytesN<32>> = crate::test_spec_hashes(&env, &amounts);
    let auto_release_seconds = 604_800u64;

    let contract_id = env.register(
        MilestoneEscrow,
        (
            admin.clone(),
            client.clone(),
            freelancer.clone(),
            arbiter.clone(),
            token.clone(),
            auto_release_seconds,
            amounts.clone(),
            spec_hashes.clone(),
        ),
    );

    let escrow = MilestoneEscrowClient::new(&env, &contract_id);
    let job = escrow.get_job();

    assert_eq!(job.client, client);
    assert_eq!(job.freelancer, freelancer);
    assert_eq!(job.arbiter, arbiter);
    assert_eq!(job.token, token);
    assert_eq!(job.auto_release_seconds, auto_release_seconds);
    assert_eq!(job.milestones.len(), 2);
    assert_eq!(job.milestones.get(0).unwrap().amount, 400_i128);
    assert_eq!(job.milestones.get(1).unwrap().amount, 600_i128);
    assert!(!job.funded);

    assert_eq!(escrow.version(), 1u32);
    assert_eq!(escrow.get_funding_mode(), crate::FundingMode::Upfront);
    assert!(escrow.get_whitelisted_tokens().contains(&token));

    let hash_0 = escrow.get_milestone_spec_hash(&0);
    assert_eq!(hash_0, spec_hashes.get(0).unwrap());

    let hash_1 = escrow.get_milestone_spec_hash(&1);
    assert_eq!(hash_1, spec_hashes.get(1).unwrap());
}

#[test]
#[should_panic]
fn deploy_without_constructor_args_fails() {
    let env = test_env();
    // Registering without required constructor arguments must fail
    env.register(MilestoneEscrow, ());
}
