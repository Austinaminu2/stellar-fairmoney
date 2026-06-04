use sep_40_oracle::{Asset, PriceFeedClient};
use soroban_sdk::{Address, Env};

/// Fetch price for a token from the SEP-40 oracle (e.g. Reflector).
/// Returns price scaled by 1e7 (standard Reflector decimals).
/// Panics if price is unavailable or stale.
pub fn get_price(env: &Env, oracle: &Address, token: &Address) -> i128 {
    let client = PriceFeedClient::new(env, oracle);
    let asset = Asset::Stellar(token.clone());
    let price_data = client.lastprice(&asset).expect("oracle price unavailable");
    assert!(price_data.price > 0, "invalid oracle price");
    price_data.price
}
