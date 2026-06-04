#![cfg(test)]

use sep_40_oracle::testutils::{Asset as OracleAsset, MockPriceOracleClient, MockPriceOracleWASM};
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

use crate::LendingContractClient;

fn create_token<'a>(
    env: &Env,
    admin: &Address,
) -> (Address, TokenClient<'a>, StellarAssetClient<'a>) {
    let addr = env.register_stellar_asset_contract_v2(admin.clone()).address();
    (
        addr.clone(),
        TokenClient::new(env, &addr),
        StellarAssetClient::new(env, &addr),
    )
}

fn setup_oracle<'a>(env: &Env, admin: &Address) -> (Address, MockPriceOracleClient<'a>) {
    let addr = env.register(MockPriceOracleWASM, ());
    let client = MockPriceOracleClient::new(env, &addr);
    // Start with empty asset list — callers configure it before use
    client.set_data(
        admin,
        &OracleAsset::Other(soroban_sdk::symbol_short!("USD")),
        &soroban_sdk::vec![env],
        &7,   // decimals
        &300, // resolution (seconds)
    );
    (addr, client)
}

/// Register a single token with a price (overwrites asset list to just this token)
fn set_price(oracle: &MockPriceOracleClient, admin: &Address, env: &Env, token: &Address, price: i128) {
    oracle.set_data(
        admin,
        &OracleAsset::Other(soroban_sdk::symbol_short!("USD")),
        &soroban_sdk::vec![env, OracleAsset::Stellar(token.clone())],
        &7,
        &300,
    );
    oracle.set_price_stable(&soroban_sdk::vec![env, price]);
}

/// Register two tokens and their prices at once
fn set_two_prices(
    oracle: &MockPriceOracleClient,
    admin: &Address,
    env: &Env,
    token_a: &Address, price_a: i128,
    token_b: &Address, price_b: i128,
) {
    oracle.set_data(
        admin,
        &OracleAsset::Other(soroban_sdk::symbol_short!("USD")),
        &soroban_sdk::vec![env, OracleAsset::Stellar(token_a.clone()), OracleAsset::Stellar(token_b.clone())],
        &7,
        &300,
    );
    oracle.set_price_stable(&soroban_sdk::vec![env, price_a, price_b]);
}

struct Setup<'a> {
    env: Env,
    admin: Address,
    treasury: Address,
    client: LendingContractClient<'a>,
    oracle: MockPriceOracleClient<'a>,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);
    let (oracle_addr, oracle) = setup_oracle(&env, &admin);
    let contract_id = env.register(crate::LendingContract, ());
    let client = LendingContractClient::new(&env, &contract_id);
    client.initialize(&admin, &treasury, &oracle_addr, &50); // 0.5% fee
    Setup { env, admin, treasury, client, oracle }
}

fn add_reserve(s: &Setup, token: &Address) {
    s.client.add_reserve(
        token, &7500, &8000, &500, &8000, &200, &300, &2000,
    );
}

#[test]
fn test_deposit_and_withdraw() {
    let s = setup();
    let user = Address::generate(&s.env);
    let (token_addr, token, asset) = create_token(&s.env, &s.admin);
    asset.mint(&user, &1_000_000);
    // price = $1.00 scaled by 1e7
    set_price(&s.oracle, &s.admin, &s.env, &token_addr, 10_000_000);
    add_reserve(&s, &token_addr);

    s.client.deposit(&user, &token_addr, &500_000);
    assert_eq!(token.balance(&user), 500_000);

    s.client.withdraw(&user, &token_addr, &200_000);
    assert_eq!(token.balance(&user), 700_000);
}

#[test]
fn test_borrow_and_repay() {
    let s = setup();
    let user = Address::generate(&s.env);
    let seeder = Address::generate(&s.env);
    let (col_addr, _, col_asset) = create_token(&s.env, &s.admin);
    let (bor_addr, bor_token, bor_asset) = create_token(&s.env, &s.admin);

    col_asset.mint(&user, &1_000_000);
    bor_asset.mint(&seeder, &2_000_000);

    set_two_prices(&s.oracle, &s.admin, &s.env, &col_addr, 10_000_000, &bor_addr, 10_000_000);
    add_reserve(&s, &col_addr);
    add_reserve(&s, &bor_addr);

    s.client.deposit(&seeder, &bor_addr, &2_000_000);
    s.client.deposit(&user, &col_addr, &1_000_000);

    // Borrow 75% of collateral value
    s.client.borrow(&user, &col_addr, &bor_addr, &750_000);
    assert_eq!(bor_token.balance(&user), 750_000);

    s.client.repay(&user, &bor_addr, &750_000);
    assert_eq!(bor_token.balance(&user), 0);
}

#[test]
#[should_panic]
fn test_borrow_exceeds_ltv() {
    let s = setup();
    let user = Address::generate(&s.env);
    let seeder = Address::generate(&s.env);
    let (col_addr, _, col_asset) = create_token(&s.env, &s.admin);
    let (bor_addr, _, bor_asset) = create_token(&s.env, &s.admin);

    col_asset.mint(&user, &1_000_000);
    bor_asset.mint(&seeder, &2_000_000);

    set_two_prices(&s.oracle, &s.admin, &s.env, &col_addr, 10_000_000, &bor_addr, 10_000_000);
    add_reserve(&s, &col_addr);
    add_reserve(&s, &bor_addr);
    s.client.deposit(&seeder, &bor_addr, &2_000_000);
    s.client.deposit(&user, &col_addr, &1_000_000);

    // 90% borrow > 75% LTV → should panic
    s.client.borrow(&user, &col_addr, &bor_addr, &900_000);
}

#[test]
fn test_liquidation() {
    let s = setup();
    let borrower = Address::generate(&s.env);
    let liquidator = Address::generate(&s.env);
    let seeder = Address::generate(&s.env);
    let (col_addr, _, col_asset) = create_token(&s.env, &s.admin);
    let (bor_addr, bor_token, bor_asset) = create_token(&s.env, &s.admin);

    col_asset.mint(&borrower, &1_000_000);
    bor_asset.mint(&seeder, &2_000_000);
    bor_asset.mint(&liquidator, &1_000_000);

    set_two_prices(&s.oracle, &s.admin, &s.env, &col_addr, 10_000_000, &bor_addr, 10_000_000);
    add_reserve(&s, &col_addr);
    add_reserve(&s, &bor_addr);

    s.client.deposit(&seeder, &bor_addr, &2_000_000);
    s.client.deposit(&borrower, &col_addr, &1_000_000);
    s.client.borrow(&borrower, &col_addr, &bor_addr, &750_000);

    // Borrow price rises to $1.20 → borrow_value = $900k > threshold $800k
    set_two_prices(&s.oracle, &s.admin, &s.env, &col_addr, 10_000_000, &bor_addr, 12_000_000);

    let repay = 375_000i128; // 50% of 750k
    s.client.liquidate(&liquidator, &borrower, &col_addr, &bor_addr, &repay);

    assert!(bor_token.balance(&liquidator) < 1_000_000);
}

#[test]
fn test_pause_blocks_actions() {
    let s = setup();
    let user = Address::generate(&s.env);
    let (token_addr, _, asset) = create_token(&s.env, &s.admin);
    asset.mint(&user, &1_000_000);
    set_price(&s.oracle, &s.admin, &s.env, &token_addr, 10_000_000);
    add_reserve(&s, &token_addr);

    s.client.set_paused(&true);

    // Create a fresh env + client to verify paused state is stored
    // (can't use catch_unwind with Soroban Env — use should_panic test instead)
    assert!(s.client.try_deposit(&user, &token_addr, &100_000).is_err());
}

#[test]
fn test_protocol_fee_collected() {
    let s = setup();
    let borrower = Address::generate(&s.env);
    let seeder = Address::generate(&s.env);
    let (col_addr, _, col_asset) = create_token(&s.env, &s.admin);
    let (bor_addr, bor_token, bor_asset) = create_token(&s.env, &s.admin);

    col_asset.mint(&borrower, &10_000_000);
    bor_asset.mint(&seeder, &10_000_000);

    set_two_prices(&s.oracle, &s.admin, &s.env, &col_addr, 10_000_000, &bor_addr, 10_000_000);
    add_reserve(&s, &col_addr);
    add_reserve(&s, &bor_addr);

    s.client.deposit(&seeder, &bor_addr, &10_000_000);
    s.client.deposit(&borrower, &col_addr, &10_000_000);

    // Borrow at 70% utilization to generate interest
    s.client.borrow(&borrower, &col_addr, &bor_addr, &7_000_000);

    // Advance ledgers significantly to accrue interest
    s.env.ledger().with_mut(|l| l.sequence_number += 5_000_000);

    // Repay triggers interest accrual + fee collection
    s.client.repay(&borrower, &bor_addr, &7_000_000);

    // Treasury should have received protocol fees (0.5% of interest)
    let treasury_balance = bor_token.balance(&s.treasury);
    assert!(treasury_balance > 0, "treasury should have collected fees");
}
