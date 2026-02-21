# Solana Escrow Engine 🔒

> **Rebuild Production Backend as On-Chain Rust Programs** — Superteam Bounty Submission  
> An escrow service built entirely on Solana, demonstrating how a traditional Web2 payment-holding backend can be re-architected as a trustless on-chain program.

---

## Live Demo

| Resource | Link |
|---|---|
| **Program (Devnet)** | [`9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF`](https://explorer.solana.com/address/9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF?cluster=devnet) |
| **Devnet Transactions** | See [Transaction Log](#devnet-transaction-log) |

---

## Table of Contents

1. [What is an Escrow Engine?](#what-is-an-escrow-engine)
2. [How It Works in Web2](#how-it-works-in-web2)
3. [How It Works on Solana](#how-it-works-on-solana)
4. [Architecture Deep Dive](#architecture-deep-dive)
5. [State Machine](#state-machine)
6. [Account Model](#account-model)
7. [Tradeoffs & Constraints](#tradeoffs--constraints)
8. [Getting Started](#getting-started)
9. [CLI Usage](#cli-usage)
10. [Running Tests](#running-tests)
11. [Devnet Transaction Log](#devnet-transaction-log)

---

## What is an Escrow Engine?

An escrow engine is a financial intermediary that holds funds on behalf of two parties (buyer and seller) until predefined conditions are met. It is one of the most fundamental patterns in commerce, marketplaces, and financial systems.

**Web2 examples:**
- eBay buyer protection — holds payment until delivery confirmed
- Upwork/Fiverr — holds freelancer payment until work is accepted
- Real estate closing — title company holds funds until deed transfer
- Amazon Pay — holds merchant payment pending order fulfillment

---

## How It Works in Web2

```
┌─────────────────────────────────────────────────────────────┐
│                    WEB2 ESCROW ARCHITECTURE                  │
│                                                             │
│  Buyer ──────────► Bank API ──────────► Holding Account     │
│                                              │               │
│  [Postgres DB]                               │               │
│  escrows table:                              ▼               │
│  { id, buyer_id, seller_id,          ┌─────────────┐        │
│    arbiter_id, amount, status }       │  Trust Layer │        │
│                                       │  (Your Co.) │        │
│  Status transitions:                  └──────┬──────┘        │
│  pending → funded → released/refunded        │               │
│                    └────────────────► Seller Account        │
└─────────────────────────────────────────────────────────────┘
```

### Web2 Components:
| Layer | Technology | Role |
|---|---|---|
| **API** | REST / Express.js / Django | Receives instructions |
| **Database** | PostgreSQL / MySQL | Stores escrow state |
| **Auth** | JWT / OAuth | Identifies who can call what |
| **Payment** | Stripe / ACH / Wire | Moves actual money |
| **Trust** | The company itself | Holds funds, enforces rules |
| **Disputes** | Support team + DB updates | Human-reviewed, slow |

### Web2 Limitations:
- **Counterparty risk**: The escrow company can be hacked, go bankrupt, or defraud users
- **Slow settlement**: Bank transfers take 1-10 business days
- **Geographic restrictions**: Not all payment methods work globally
- **High fees**: Card surcharges + platform fees (2-5%)
- **Opacity**: Users must trust the platform's internal state

---

## How It Works on Solana

```
┌─────────────────────────────────────────────────────────────┐
│                   SOLANA ESCROW ARCHITECTURE                 │
│                                                             │
│  Buyer ──── signs tx ────► Escrow Program (Rust)            │
│                                    │                        │
│  On-chain accounts:                ▼                        │
│  EscrowAccount PDA ────► VaultAccount PDA                   │
│  [buyer, seller,          [holds actual SOL]                │
│   arbiter, state]                  │                        │
│                                    ▼                        │
│  State transitions:         Seller's Wallet                 │
│  Active → Funded → Released                                 │
│                  └── Refunded                               │
│         └── Disputed                                        │
└─────────────────────────────────────────────────────────────┘
```

### Solana Components:
| Layer | Solana Equivalent | Role |
|---|---|---|
| **API** | Program instructions | `initialize_escrow`, `deposit`, `release`, etc. |
| **Database** | `EscrowAccount` PDA | On-chain data storage (replaces PostgreSQL) |
| **Auth** | Signer verification + PDA seeds | Who can call what — enforced in Rust, not middleware |
| **Payment** | `system_program::transfer` CPI | Move SOL atomically at network speed |
| **Trust** | Program logic itself | No company needed — code IS the escrow |
| **Disputes** | On-chain `arbiter` pubkey | Cryptographic identity, instant execution |

### Solana Advantages:
- **Trustless**: No company holds your funds. Math does.
- **Instant settlement**: Sub-second finality (~400ms)
- **Global by default**: Anyone with a wallet participates
- **Dirt cheap**: ~$0.00025 per transaction
- **Fully auditable**: Every state change on public blockchain
- **Atomic execution**: Either the whole tx succeeds or nothing changes

---

## Architecture Deep Dive

### Program Derived Addresses (PDAs) vs. Database Rows

In Web2, each escrow is a row in a database table with a `UUID` primary key assigned by the server. You query it via `SELECT * FROM escrows WHERE id = $1`.

In Solana, each escrow is a **PDA** — an account whose address is deterministically derived from:

```
EscrowAccount address = hash("escrow" || buyer_pubkey || escrow_id)
VaultAccount address  = hash("vault"  || buyer_pubkey || escrow_id)
```

This means:
- **No lookup needed** — anyone can compute the account address from public inputs
- **No collisions** — each buyer+id combination maps to a unique address
- **Ownership guarantees** — only the escrow program can modify these accounts

```
Web2:  SELECT * FROM escrows WHERE buyer = ? AND id = ?
            ↓
Solana: findProgramAddressSync(["escrow", buyer, escrow_id], programId)
```

### Instructions vs. REST Endpoints

| REST API | Solana Instruction | Who Can Call |
|---|---|---|
| `POST /escrows` | `initialize_escrow` | Anyone (buyer) |
| `PUT /escrows/:id/deposit` | `deposit` | Buyer only |
| `POST /escrows/:id/release` | `release` | Buyer or Arbiter |
| `POST /escrows/:id/refund` | `refund` | Seller or Arbiter |
| `POST /escrows/:id/dispute` | `dispute` | Buyer or Seller |

In Web2, authorization is middleware: `if (req.user.id !== escrow.buyer_id) return 403`.

In Solana, authorization is enforced in Rust at the account level:

```rust
// This is the Solana equivalent of JWT middleware:
require!(
    ctx.accounts.caller.key() == escrow.buyer || 
    ctx.accounts.caller.key() == escrow.arbiter,
    EscrowError::Unauthorized
);
```

The difference: Solana's check runs in a distributed, trustless environment where the runtime verifies signatures before the program even executes.

### The Vault: Program-Owned Holding Account

In Web2, "holding funds" means a record in Stripe saying `amount=100, status=held`. The actual money is in Stripe's bank.

In Solana, the **vault is a real account** (`VaultAccount` PDA) that literally holds SOL in its lamport balance. The account is owned by our program, meaning only our program logic can debit it:

```rust
// Only this program can move funds from vault (no middleware, no admin panel)
ctx.accounts.vault.sub_lamports(escrow.amount)?;
ctx.accounts.seller.add_lamports(escrow.amount)?;
```

This is enforced by the Solana runtime at the hardware level — no code can bypass it.

---

## State Machine

```
                    initialize_escrow()
                          │
                          ▼
                      ┌────────┐
                      │ ACTIVE │  ← Escrow created, no funds yet
                      └────┬───┘
                           │ deposit() [buyer signs]
                           ▼
                      ┌────────┐
                      │ FUNDED │  ← SOL locked in vault PDA
                      └──┬──┬──┘
                         │  │
         release()        │  │  dispute()
    [buyer / arbiter]     │  │  [buyer / seller]
                         │  │
              ┌──────────┘  └──────────────┐
              │                            │
              ▼                            ▼
        ┌──────────┐                ┌──────────┐
        │ RELEASED │                │ DISPUTED │
        └──────────┘                └────┬─────┘
                                         │
              refund()                   │  release() or refund()
         [seller / arbiter]              │  [arbiter only]
                                         │
              ┌──────────────────────────┤
              │                          │
              ▼                          ▼
        ┌──────────┐              ┌──────────┐
        │ REFUNDED │              │ RELEASED │
        └──────────┘              └──────────┘
```

---

## Account Model

### EscrowAccount (data storage)

| Field | Type | Size | Description |
|---|---|---|---|
| `discriminator` | `[u8; 8]` | 8 | Anchor account type identifier |
| `escrow_id` | `u64` | 8 | Unique ID chosen by buyer |
| `buyer` | `Pubkey` | 32 | Buyer's wallet address |
| `seller` | `Pubkey` | 32 | Seller's wallet address |
| `arbiter` | `Pubkey` | 32 | Neutral arbiter address |
| `amount` | `u64` | 8 | Locked amount in lamports |
| `description` | `String` | 4+200 | Agreement description (max 200 chars) |
| `state` | `EscrowState` | 1 | Current lifecycle state |
| `created_at` | `i64` | 8 | Unix timestamp |
| `escrow_bump` | `u8` | 1 | PDA canonical bump |
| `vault_bump` | `u8` | 1 | Vault PDA canonical bump |
| **Total** | | **335 bytes** | |

### VaultAccount (SOL holder)

| Field | Type | Size | Description |
|---|---|---|---|
| `discriminator` | `[u8; 8]` | 8 | Anchor account type identifier |
| **SOL balance** | *lamports* | — | Escrowed SOL (in account lamports, not data) |
| **Total** | | **8 bytes** | |

The vault holds the escrowed SOL as native lamports. Since it's owned by the program, no external account can debit it — only the program's release/refund logic can transfer funds out.

---

## Tradeoffs & Constraints

### ✅ What Solana Does Better

| Concern | Web2 | Solana |
|---|---|---|
| **Trust** | Trust the company | Trust the math |
| **Settlement speed** | 1-3 business days | ~400ms |
| **Global access** | KYC, bank restrictions | Open to any wallet |
| **Auditability** | Private DB, FOIA needed | Public blockchain |
| **Censorship** | Company can freeze funds | Program can't be modified after deploy |
| **Cost** | 2-5% fees | ~$0.00025/tx |

### ⚠️ What Web2 Does Better

| Concern | Web2 | Solana |
|---|---|---|
| **Legal recourse** | Courts, chargebacks | Users bear full responsibility |
| **Fiat currency** | Native fiat (USD, EUR) | SOL + stablecoins only |
| **User experience** | Simple UI, email login | Wallet required, seed phrases |
| **Error recovery** | Support team can fix mistakes | Immutable — bugs are permanent |
| **Complex logic** | Any code, any DB | Compute unit limits (200k CU/tx) |
| **Off-chain data** | Rich metadata, attachments | 10KB account size limit |
| **Privacy** | Private by default | Public by default |

### 🔧 Technical Constraints on Solana

1. **Account size is fixed at creation** — can't grow the `description` field later
2. **No dynamic dispatch** — can't call arbitrary programs (only known CPI targets)
3. **Compute budget** — complex arbiter logic (e.g., multi-sig) must stay within 200k CU
4. **No off-chain triggers** — can't auto-release after 30 days without a crank/oracle
5. **SOL only in this impl** — SPL token support requires `anchor-spl` (extension possible)
6. **Rent requirement** — accounts must maintain minimum lamport balance to stay alive

### 🔮 Production Extensions

To take this to production:
- **SPL tokens** — use `anchor-spl` for USDC/USDT escrow
- **Timelock** — integrate Clockwork/Drift to auto-release after N days
- **Multi-sig arbiter** — 2-of-3 arbiter committee via Squads protocol
- **Privacy** — store only a Merkle root on-chain, full data off-chain (Arweave)
- **Fee collection** — add a 0.1% protocol fee instruction for sustainability

---

## Getting Started

### Prerequisites

```bash
# Required tools
rustc --version      # >= 1.75
solana --version     # >= 1.18
anchor --version     # >= 0.32
node --version       # >= 18
```

### Install & Build

```bash
git clone https://github.com/S1ngle777/solana-escrow-engine
cd solana-escrow-engine

npm install
anchor build
```

### Configure Wallet

```bash
# Generate a new devnet wallet (or use existing)
solana-keygen new --outfile ~/.config/solana/id.json
solana config set --url devnet

# Fund with devnet SOL
solana airdrop 2
solana balance
```

---

## CLI Usage

The CLI requires a built IDL. Run `anchor build` first.

```bash
# Shorthand
alias escrow="npx ts-node client/cli.ts"

# 1. Create an escrow
CLUSTER=devnet escrow init \
  --id 42 \
  --seller <SELLER_PUBKEY> \
  --arbiter <ARBITER_PUBKEY> \
  --amount 0.5 \
  --desc "Payment for logo design"

# 2. Deposit funds (buyer wallet signs)
CLUSTER=devnet escrow deposit --id 42 --buyer <BUYER_PUBKEY>

# 3a. Buyer is satisfied — release to seller
CLUSTER=devnet escrow release --id 42 --buyer <BUYER_PUBKEY> --seller <SELLER_PUBKEY>

# 3b. Buyer unhappy — open dispute
CLUSTER=devnet escrow dispute --id 42 --buyer <BUYER_PUBKEY>

# 3c. Seller agrees to refund
CLUSTER=devnet escrow refund --id 42 --buyer <BUYER_PUBKEY>

# Check current status at any time
CLUSTER=devnet escrow status --id 42 --buyer <BUYER_PUBKEY>
```

**Output example for `status`:**
```
📋 Escrow #42 Status
  State:       FUNDED (in escrow)
  Buyer:       7xKX...abc
  Seller:      9mPR...xyz
  Arbiter:     3kAB...mnp
  Amount:      0.5 SOL (500000000 lamports)
  Vault:       0.5 SOL held in PDA
  Description: Payment for logo design
  Created at:  2026-02-21T14:00:00.000Z

  🔍 View on Explorer: https://explorer.solana.com/address/...?cluster=devnet
```

---

## Running Tests

Tests run against a local validator (spawned automatically by Anchor):

```bash
# Run all tests (uses local validator)
anchor test

# Expected output:
#   solana-escrow-engine
#     ✓ 1. Initializes an escrow (buyer creates PDA accounts)
#     ✓ 2. Deposits SOL into vault (buyer funds the escrow)
#     ✓ 3. Rejects dispute from an outsider (access control check)
#     ✓ 4. Buyer opens a dispute (state transition: Funded → Disputed)
#     ✓ 5. Arbiter releases funds to seller (dispute resolution)
#     ✓ 6. Seller voluntarily refunds buyer (happy path refund)
#     ✓ 7. Rejects double-deposit on already funded escrow (state guard)
#     ✓ 8. Buyer releases funds directly to seller (happy path)
#
#   8 passing
```

### Test Coverage

| Test | Scenario | Checks |
|---|---|---|
| 1 | Initialize escrow | PDA created, all fields correct |
| 2 | Deposit | Vault balance increases, state = Funded |
| 3 | Unauthorized dispute | Error = Unauthorized |
| 4 | Dispute opening | State = Disputed |
| 5 | Arbiter releases | Seller receives SOL, state = Released |
| 6 | Seller refunds | Buyer receives SOL, state = Refunded |
| 7 | Double-deposit | Error = InvalidState |
| 8 | Buyer self-release | Direct happy path, no dispute |

---

## Devnet Transaction Log

> All transactions executed on Solana Devnet — publicly verifiable on Explorer.

| Action | Transaction Signature | Explorer Link |
|---|---|---|
| **Program Deploy** | `RCjdWhim...` | [View](https://explorer.solana.com/tx/RCjdWhimdFD78YP4pSSC98bLp28u4csDQ41JiEjFowM9EWjYHknLGK1AU7Msi1JdsnowGXtH1Hhj7mPGcvn7zXR?cluster=devnet) |
| **initializeEscrow** | `64hY5cYJ...` | [View](https://explorer.solana.com/tx/64hY5cYJDMKzqssU6msSY5YXnSh7TnAeDWATEr1R1U1uNtVNsRU9puX6SY4pxE5z2FtsN8DuVfYddcQZFJiwbTX4?cluster=devnet) |
| **deposit** | `5nEUbtWW...` | [View](https://explorer.solana.com/tx/5nEUbtWWR89qF351YYBWzGQUhEpgi4b7pDEM265xdXJP5d8Mrg7XxovG4BdoaUeQTP9GfpXgBuLnpk4bED39s2kK?cluster=devnet) |
| **release** | `65fJ1CV8...` | [View](https://explorer.solana.com/tx/65fJ1CV875HTkWbcUfz4sLyzSvGpW9ZhGNpVK3d7QLacQcvVNAwLD78ff9qSU9XvPan8MqmWmAkgS6pQPvPfe9fW?cluster=devnet) |

**Program address**: [`9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF`](https://explorer.solana.com/address/9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF?cluster=devnet)

The demo executed a full escrow lifecycle on-chain:
1. `initializeEscrow` — created EscrowAccount + VaultAccount PDAs
2. `deposit` — locked 0.01 SOL in vault PDA  
3. `release` — buyer approved, 0.01 SOL transferred to seller instantly

---

## Project Structure

```
solana-escrow-engine/
├── programs/
│   └── solana-escrow-engine/
│       └── src/
│           └── lib.rs          # Main Anchor program (all instructions + state)
├── tests/
│   └── solana-escrow-engine.ts # 8 comprehensive integration tests
├── client/
│   └── cli.ts                  # TypeScript CLI for human interaction
├── Anchor.toml                 # Anchor config (devnet)
├── Cargo.toml                  # Rust workspace
├── package.json                # Node dependencies
└── README.md                   # This file
```

---

## License

MIT — free to use, fork, and learn from.

---

*Built for the Superteam "Rebuild Production Backend Systems as On-Chain Rust Programs" bounty.*
