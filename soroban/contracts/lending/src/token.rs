use soroban_sdk::{Address, Env};

pub fn transfer_from(env: &Env, token: &Address, from: &Address, to: &Address, amount: i128) {
    let client = soroban_sdk::token::Client::new(env, token);
    client.transfer(from, to, &amount);
}

pub fn transfer_to_user(env: &Env, token: &Address, user: &Address, amount: i128) {
    let client = soroban_sdk::token::Client::new(env, token);
    client.transfer(&env.current_contract_address(), user, &amount);
}

pub fn balance(env: &Env, token: &Address, account: &Address) -> i128 {
    let client = soroban_sdk::token::Client::new(env, token);
    client.balance(account)
}
