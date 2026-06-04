mod errors;
mod interest;
mod oracle;
mod storage;
mod tests;
mod token;

use soroban_sdk::{contract, contractimpl, Address, Env};

use interest::{accrue_index, amount_to_shares, shares_to_amount};
use oracle::get_price;
use storage::{
    get_market, get_reserve_config, get_reserve_data, get_user_position, is_initialized,
    set_market, set_reserve_config, set_reserve_data, set_user_position, MarketConfig,
    ReserveConfig, ReserveData,
};
use token::{transfer_from, transfer_to_user};

#[contract]
pub struct LendingContract;

#[contractimpl]
impl LendingContract {
    // -------------------------------------------------------------------------
    // Admin
    // -------------------------------------------------------------------------

    /// Initialize the lending market
    pub fn initialize(
        env: Env,
        admin: Address,
        treasury: Address,
        oracle: Address,
        protocol_fee_bps: u32,
    ) {
        assert!(!is_initialized(&env), "already initialized");
        set_market(
            &env,
            &MarketConfig {
                admin,
                treasury,
                protocol_fee_bps,
                is_paused: false,
                oracle,
            },
        );
    }

    /// Pause or unpause the entire market
    pub fn set_paused(env: Env, paused: bool) {
        let mut market = get_market(&env);
        market.admin.require_auth();
        market.is_paused = paused;
        set_market(&env, &market);
    }

    /// Update the oracle address
    pub fn set_oracle(env: Env, oracle: Address) {
        let mut market = get_market(&env);
        market.admin.require_auth();
        market.oracle = oracle;
        set_market(&env, &market);
    }

    /// Update protocol fee
    pub fn set_protocol_fee(env: Env, fee_bps: u32) {
        let mut market = get_market(&env);
        market.admin.require_auth();
        market.protocol_fee_bps = fee_bps;
        set_market(&env, &market);
    }

    /// Register a new reserve (token market)
    pub fn add_reserve(
        env: Env,
        token: Address,
        ltv_bps: u32,
        liquidation_threshold_bps: u32,
        liquidation_bonus_bps: u32,
        optimal_utilization_bps: u32,
        base_rate_bps: u32,
        slope1_bps: u32,
        slope2_bps: u32,
    ) {
        let market = get_market(&env);
        market.admin.require_auth();
        assert!(!market.is_paused, "market is paused");

        set_reserve_config(
            &env,
            &token,
            &ReserveConfig {
                ltv_bps,
                liquidation_threshold_bps,
                liquidation_bonus_bps,
                optimal_utilization_bps,
                base_rate_bps,
                slope1_bps,
                slope2_bps,
                is_active: true,
            },
        );
        set_reserve_data(
            &env,
            &token,
            &ReserveData {
                total_deposits: 0,
                total_borrows: 0,
                borrow_index: 1_000_000_000,
                last_update_ledger: env.ledger().sequence(),
            },
        );
    }

    /// Activate or deactivate a reserve
    pub fn set_reserve_active(env: Env, token: Address, active: bool) {
        let market = get_market(&env);
        market.admin.require_auth();
        let mut config = get_reserve_config(&env, &token).expect("reserve not found");
        config.is_active = active;
        set_reserve_config(&env, &token, &config);
    }

    // -------------------------------------------------------------------------
    // User actions
    // -------------------------------------------------------------------------

    /// Deposit tokens to earn yield
    pub fn deposit(env: Env, user: Address, token: Address, amount: i128) {
        user.require_auth();
        assert!(amount > 0, "amount must be > 0");
        let market = get_market(&env);
        assert!(!market.is_paused, "market is paused");

        let config = get_reserve_config(&env, &token).expect("reserve not found");
        assert!(config.is_active, "reserve not active");

        transfer_from(&env, &token, &user, &env.current_contract_address(), amount);

        let mut data = get_reserve_data(&env, &token).unwrap();
        // accrue interest before changing state
        let (new_index, interest) = accrue_index(&data, &config, env.ledger().sequence());
        let fee = Self::collect_fee(&env, &token, &market, &mut data, interest);
        data.total_deposits += amount - fee; // fee leaves the pool
        data.borrow_index = new_index;
        data.last_update_ledger = env.ledger().sequence();
        set_reserve_data(&env, &token, &data);

        let mut pos = get_user_position(&env, &user, &token).unwrap_or_default();
        pos.deposited += amount;
        set_user_position(&env, &user, &token, &pos);
    }

    /// Withdraw deposited tokens
    pub fn withdraw(env: Env, user: Address, token: Address, amount: i128) {
        user.require_auth();
        assert!(amount > 0, "amount must be > 0");
        let market = get_market(&env);
        assert!(!market.is_paused, "market is paused");

        let config = get_reserve_config(&env, &token).expect("reserve not found");
        let mut data = get_reserve_data(&env, &token).unwrap();
        let (new_index, interest) = accrue_index(&data, &config, env.ledger().sequence());
        let fee = Self::collect_fee(&env, &token, &market, &mut data, interest);

        let mut pos = get_user_position(&env, &user, &token).unwrap_or_default();
        assert!(pos.deposited >= amount, "insufficient deposit");

        let available = data.total_deposits.saturating_sub(data.total_borrows + fee);
        assert!(available >= amount, "insufficient liquidity");

        transfer_to_user(&env, &token, &user, amount);

        data.total_deposits -= amount + fee;
        data.borrow_index = new_index;
        data.last_update_ledger = env.ledger().sequence();
        set_reserve_data(&env, &token, &data);

        pos.deposited -= amount;
        set_user_position(&env, &user, &token, &pos);
    }

    /// Borrow tokens against collateral — uses oracle for pricing
    pub fn borrow(
        env: Env,
        user: Address,
        collateral_token: Address,
        borrow_token: Address,
        amount: i128,
    ) {
        user.require_auth();
        assert!(amount > 0, "amount must be > 0");
        let market = get_market(&env);
        assert!(!market.is_paused, "market is paused");

        let collateral_config =
            get_reserve_config(&env, &collateral_token).expect("collateral reserve not found");
        let borrow_config =
            get_reserve_config(&env, &borrow_token).expect("borrow reserve not found");
        assert!(borrow_config.is_active, "reserve not active");

        // Oracle prices scaled by 1e7
        let collateral_price = get_price(&env, &market.oracle, &collateral_token);
        let borrow_price = get_price(&env, &market.oracle, &borrow_token);

        let collateral_pos =
            get_user_position(&env, &user, &collateral_token).unwrap_or_default();

        let collateral_value = collateral_pos.deposited * collateral_price / 10_000_000;
        let max_borrow_value = collateral_value * collateral_config.ltv_bps as i128 / 10_000;

        let mut borrow_data = get_reserve_data(&env, &borrow_token).unwrap();
        let (new_index, interest) =
            accrue_index(&borrow_data, &borrow_config, env.ledger().sequence());
        Self::collect_fee(&env, &borrow_token, &market, &mut borrow_data, interest);

        let borrow_pos = get_user_position(&env, &user, &borrow_token).unwrap_or_default();
        let existing_borrow = shares_to_amount(borrow_pos.borrow_shares, new_index);
        let new_borrow_value = (existing_borrow + amount) * borrow_price / 10_000_000;

        assert!(new_borrow_value <= max_borrow_value, "insufficient collateral");

        let available = borrow_data.total_deposits.saturating_sub(borrow_data.total_borrows);
        assert!(available >= amount, "insufficient liquidity");

        transfer_to_user(&env, &borrow_token, &user, amount);

        borrow_data.total_borrows += amount;
        borrow_data.borrow_index = new_index;
        borrow_data.last_update_ledger = env.ledger().sequence();
        set_reserve_data(&env, &borrow_token, &borrow_data);

        let mut pos = borrow_pos;
        let new_shares = amount_to_shares(amount, new_index);
        pos.borrow_shares += new_shares;
        pos.last_update_ledger = env.ledger().sequence();
        set_user_position(&env, &user, &borrow_token, &pos);
    }

    /// Repay borrowed tokens
    pub fn repay(env: Env, user: Address, token: Address, amount: i128) {
        user.require_auth();
        assert!(amount > 0, "amount must be > 0");
        let market = get_market(&env);
        assert!(!market.is_paused, "market is paused");

        let config = get_reserve_config(&env, &token).expect("reserve not found");
        let mut data = get_reserve_data(&env, &token).unwrap();
        let (new_index, interest) = accrue_index(&data, &config, env.ledger().sequence());
        Self::collect_fee(&env, &token, &market, &mut data, interest);

        let mut pos = get_user_position(&env, &user, &token).unwrap_or_default();
        assert!(pos.borrow_shares > 0, "nothing to repay");

        let total_owed = shares_to_amount(pos.borrow_shares, new_index);
        let repay_amount = amount.min(total_owed);

        transfer_from(
            &env,
            &token,
            &user,
            &env.current_contract_address(),
            repay_amount,
        );

        let shares_to_burn = amount_to_shares(repay_amount, new_index).min(pos.borrow_shares);
        data.total_borrows = (data.total_borrows - repay_amount).max(0);
        data.borrow_index = new_index;
        data.last_update_ledger = env.ledger().sequence();
        set_reserve_data(&env, &token, &data);

        pos.borrow_shares = (pos.borrow_shares - shares_to_burn).max(0);
        pos.last_update_ledger = env.ledger().sequence();
        set_user_position(&env, &user, &token, &pos);
    }

    /// Liquidate an undercollateralized position — uses oracle for pricing
    pub fn liquidate(
        env: Env,
        liquidator: Address,
        borrower: Address,
        collateral_token: Address,
        borrow_token: Address,
        repay_amount: i128,
    ) {
        liquidator.require_auth();
        assert!(repay_amount > 0, "amount must be > 0");
        let market = get_market(&env);
        assert!(!market.is_paused, "market is paused");

        let collateral_config =
            get_reserve_config(&env, &collateral_token).expect("collateral reserve not found");
        let borrow_config =
            get_reserve_config(&env, &borrow_token).expect("borrow reserve not found");

        let collateral_price = get_price(&env, &market.oracle, &collateral_token);
        let borrow_price = get_price(&env, &market.oracle, &borrow_token);

        let mut borrow_data = get_reserve_data(&env, &borrow_token).unwrap();
        let (new_borrow_index, borrow_interest) =
            accrue_index(&borrow_data, &borrow_config, env.ledger().sequence());
        Self::collect_fee(&env, &borrow_token, &market, &mut borrow_data, borrow_interest);

        let collateral_pos =
            get_user_position(&env, &borrower, &collateral_token).unwrap_or_default();
        let borrow_pos =
            get_user_position(&env, &borrower, &borrow_token).unwrap_or_default();

        let total_borrowed = shares_to_amount(borrow_pos.borrow_shares, new_borrow_index);

        let collateral_value = collateral_pos.deposited * collateral_price / 10_000_000;
        let borrow_value = total_borrowed * borrow_price / 10_000_000;
        let threshold_value =
            collateral_value * collateral_config.liquidation_threshold_bps as i128 / 10_000;

        assert!(borrow_value > threshold_value, "position is healthy");

        let max_repay = total_borrowed / 2;
        assert!(repay_amount <= max_repay, "repay amount too large");

        let repay_value = repay_amount * borrow_price / 10_000_000;
        let seize_value = repay_value
            * (10_000 + collateral_config.liquidation_bonus_bps as i128)
            / 10_000;
        let collateral_to_seize =
            (seize_value * 10_000_000 / collateral_price.max(1)).min(collateral_pos.deposited);

        // Liquidator repays borrower's debt
        transfer_from(
            &env,
            &borrow_token,
            &liquidator,
            &env.current_contract_address(),
            repay_amount,
        );
        // Liquidator receives collateral
        transfer_to_user(&env, &collateral_token, &liquidator, collateral_to_seize);

        // Update borrow reserve
        let shares_burned =
            amount_to_shares(repay_amount, new_borrow_index).min(borrow_pos.borrow_shares);
        borrow_data.total_borrows = (borrow_data.total_borrows - repay_amount).max(0);
        borrow_data.borrow_index = new_borrow_index;
        borrow_data.last_update_ledger = env.ledger().sequence();
        set_reserve_data(&env, &borrow_token, &borrow_data);

        // Update collateral reserve
        let mut collateral_data = get_reserve_data(&env, &collateral_token).unwrap();
        collateral_data.total_deposits =
            (collateral_data.total_deposits - collateral_to_seize).max(0);
        set_reserve_data(&env, &collateral_token, &collateral_data);

        // Update borrower positions
        let mut bp = borrow_pos;
        bp.borrow_shares = (bp.borrow_shares - shares_burned).max(0);
        set_user_position(&env, &borrower, &borrow_token, &bp);

        let mut cp = collateral_pos;
        cp.deposited = (cp.deposited - collateral_to_seize).max(0);
        set_user_position(&env, &borrower, &collateral_token, &cp);
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    /// Collect protocol fee from accrued interest, transfer to treasury, return fee amount.
    fn collect_fee(
        env: &Env,
        token: &Address,
        market: &MarketConfig,
        data: &mut ReserveData,
        interest: i128,
    ) -> i128 {
        if interest <= 0 || market.protocol_fee_bps == 0 {
            return 0;
        }
        let fee = interest * market.protocol_fee_bps as i128 / 10_000;
        if fee > 0 {
            // Only transfer if the vault actually holds enough
            let vault_balance = token::balance(env, token, &env.current_contract_address());
            if vault_balance >= fee {
                transfer_to_user(env, token, &market.treasury, fee);
            }
            // Add interest (minus fee) to deposits so depositors earn yield
            data.total_deposits += interest - fee;
        }
        fee
    }
}
