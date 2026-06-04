# FairMoney 🌊

A decentralized lending protocol built on Solana using the Anchor framework. FairMoney allows users to deposit collateral, borrow against it, earn yield on deposits, and participate in liquidations.

---

## Features

- **Multi-asset lending** — support for any SPL token
- **Collateralized borrowing** — borrow against deposited assets using configurable LTV ratios
- **Interest accrual** — two-slope interest rate model (similar to Aave/Compound)
- **Liquidations** — undercollateralized positions can be liquidated with a bonus reward
- **Mock oracle** — configurable mock prices per reserve (Pyth-ready architecture)
- **Bankrun tests** — fast, local test suite with no validator needed

---

## Architecture

```
stellar-fairmoney/
├── anchor/
│   ├── programs/stellar-fairmoney/src/
│   │   ├── lib.rs                        # Program entry point
│   │   ├── errors.rs                     # Custom error codes
│   │   ├── instructions/
│   │   │   ├── initialize_market.rs      # Create lending market
│   │   │   ├── initialize_reserve.rs     # Add token reserve
│   │   │   ├── deposit.rs                # Supply tokens
│   │   │   ├── withdraw.rs               # Withdraw supplied tokens
│   │   │   ├── borrow.rs                 # Borrow against collateral
│   │   │   ├── repay.rs                  # Repay borrowed tokens
│   │   │   └── liquidate.rs              # Liquidate unhealthy positions
│   │   └── state/
│   │       ├── market.rs                 # Global market state
│   │       ├── reserve.rs                # Per-token reserve state
│   │       └── user_position.rs          # Per-user position state
│   └── tests/
│       └── stellar-fairmoney.spec.ts           # Full Bankrun test suite
```

---

## Account Structure

### Market
Global config for the lending protocol.
- `authority` — admin pubkey
- `is_paused` — emergency pause flag
- `reserve_count` — number of active reserves
- `protocol_fee_bps` — fee taken on interest
- `treasury` — fee recipient

### Reserve
Per-token lending pool.
- `token_mint` — the SPL token
- `liquidity_vault` — vault holding deposited tokens
- `total_deposits` / `total_borrows` — pool state
- `ltv_bps` — max loan-to-value (e.g. 7500 = 75%)
- `liquidation_threshold_bps` — threshold before liquidation (e.g. 8000 = 80%)
- `liquidation_bonus_bps` — bonus for liquidators (e.g. 500 = 5%)
- `mock_price` — USD price scaled by 1e6

### UserPosition
Per-user, per-reserve position.
- `deposited_amount` — tokens supplied
- `borrowed_amount` — tokens borrowed
- `last_borrow_rate` — rate snapshot for interest calc

---

## Interest Rate Model

FairMoney uses a two-slope interest rate model:

```
If utilization <= optimal:
  rate = base_rate + slope1 * (utilization / optimal)

If utilization > optimal:
  rate = base_rate + slope1 + slope2 * ((utilization - optimal) / (1 - optimal))
```

This incentivizes utilization near the optimal rate (e.g. 80%) while making borrowing very expensive above it.

---

## PDAs

| Account | Seeds |
|---|---|
| Market | `["market", authority]` |
| Reserve | `["reserve", market, token_mint]` |
| Vault | `["vault", reserve]` |
| UserPosition | `["position", reserve, user]` |

---

## Getting Started

### Prerequisites

- Rust + Cargo
- Solana CLI
- Anchor CLI (`avm install 0.32.1`)
- Node.js + Yarn

### Install

```bash
git clone https://github.com/yourusername/stellar-fairmoney
cd stellar-fairmoney/anchor
yarn install
```

### Build

```bash
anchor build
```

### Test

```bash
anchor test --skip-local-validator --skip-deploy
```

### Deploy to Devnet

```bash
solana config set --url devnet
anchor deploy
```

---

## Instructions

| Instruction | Description |
|---|---|
| `initialize_market` | Create a new lending market |
| `initialize_reserve` | Add a new token to the market |
| `deposit` | Supply tokens to earn yield |
| `withdraw` | Remove supplied tokens |
| `borrow` | Borrow tokens against collateral |
| `repay` | Pay back borrowed tokens |
| `liquidate` | Liquidate undercollateralized positions |

---

## Health Factor

A position becomes liquidatable when:

```
borrow_value > collateral_value * liquidation_threshold
```

Liquidators can repay up to 50% of the borrow and receive collateral at a discount (liquidation bonus).

---

## License

MIT
