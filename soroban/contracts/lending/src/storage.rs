use soroban_sdk::{contracttype, Address, Env};

#[contracttype]
#[derive(Clone)]
pub struct ReserveConfig {
    pub ltv_bps: u32,
    pub liquidation_threshold_bps: u32,
    pub liquidation_bonus_bps: u32,
    pub optimal_utilization_bps: u32,
    pub base_rate_bps: u32,
    pub slope1_bps: u32,
    pub slope2_bps: u32,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct ReserveData {
    pub total_deposits: i128,
    pub total_borrows: i128,
    /// Cumulative borrow index scaled by 1e9 (starts at 1_000_000_000)
    pub borrow_index: i128,
    pub last_update_ledger: u32,
}

#[contracttype]
#[derive(Clone, Default)]
pub struct UserPosition {
    /// Raw deposited amount
    pub deposited: i128,
    /// Scaled borrow shares (borrowed / borrow_index_at_time * 1e9)
    pub borrow_shares: i128,
    pub last_update_ledger: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct MarketConfig {
    pub admin: Address,
    pub treasury: Address,
    pub protocol_fee_bps: u32,
    pub is_paused: bool,
    /// Oracle contract address (SEP-40 compatible, e.g. Reflector)
    pub oracle: Address,
}

#[contracttype]
enum DataKey {
    Market,
    ReserveConfig(Address),
    ReserveData(Address),
    UserPosition(Address, Address), // (user, token)
}

pub fn is_initialized(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Market)
}

pub fn set_market(env: &Env, config: &MarketConfig) {
    env.storage().instance().set(&DataKey::Market, config);
}

pub fn get_market(env: &Env) -> MarketConfig {
    env.storage().instance().get(&DataKey::Market).unwrap()
}

pub fn set_reserve_config(env: &Env, token: &Address, config: &ReserveConfig) {
    env.storage()
        .persistent()
        .set(&DataKey::ReserveConfig(token.clone()), config);
}

pub fn get_reserve_config(env: &Env, token: &Address) -> Option<ReserveConfig> {
    env.storage()
        .persistent()
        .get(&DataKey::ReserveConfig(token.clone()))
}

pub fn set_reserve_data(env: &Env, token: &Address, data: &ReserveData) {
    env.storage()
        .persistent()
        .set(&DataKey::ReserveData(token.clone()), data);
}

pub fn get_reserve_data(env: &Env, token: &Address) -> Option<ReserveData> {
    env.storage()
        .persistent()
        .get(&DataKey::ReserveData(token.clone()))
}

pub fn set_user_position(env: &Env, user: &Address, token: &Address, pos: &UserPosition) {
    env.storage()
        .persistent()
        .set(&DataKey::UserPosition(user.clone(), token.clone()), pos);
}

pub fn get_user_position(env: &Env, user: &Address, token: &Address) -> Option<UserPosition> {
    env.storage()
        .persistent()
        .get(&DataKey::UserPosition(user.clone(), token.clone()))
}
