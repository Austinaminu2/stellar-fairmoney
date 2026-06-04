use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LendingError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    ReserveNotFound = 3,
    InsufficientCollateral = 4,
    InsufficientLiquidity = 5,
    PositionHealthy = 6,
    ZeroAmount = 7,
}
