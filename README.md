# Solana Escrow Engine 🔒

> **Rebuild Production Backend as On-Chain Rust Programs** — Superteam Bounty Submission  
> A trustless escrow service built entirely on Solana, demonstrating how a traditional Web2 payment-holding backend can be re-architected as an on-chain program using Anchor/Rust.

---

## Live on Devnet

| Resource | Link |
|---|---|
| **Program** | [`9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF`](https://explorer.solana.com/address/9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF?cluster=devnet) |
| **Devnet Transactions** | See [Transaction Log](#devnet-transaction-log) |

---

## Table of Contents

1. [What is an Escrow Engine?](#what-is-an-escrow-engine)
2. [How It Works in Web2](#how-it-works-in-web2)
3. [How It Works on Solana](#how-it-works-on-solana)
4. [Architecture Deep Dive](#architecture-deep-dive)
5. [Design Decisions](#design-decisions)
6. [State Machine](#state-machine)
7. [Account Model](#account-model)
8. [Instruction Reference](#instruction-reference)
9. [Error Handling](#error-handling)
10. [Tradeoffs & Constraints](#tradeoffs--constraints)
11. [Getting Started](#getting-started)
12. [CLI Usage](#cli-usage)
13. [Running Tests](#running-tests)
14. [Project Structure](#project-structure)
15. [Devnet Transaction Log](#devnet-transaction-log)

---

## What is an Escrow Engine?

An escrow engine is a financial intermediary that holds funds on behalf of two parties (buyer and seller) until predefined conditions are met. It is one of the most fundamental patterns in commerce, marketplaces, and financial systems.

**Real-world escrow services:**

| Service | Pattern | Settlement |
|---|---|---|
| eBay Buyer Protection | Holds payment until delivery confirmed | 2-5 days |
| Upwork / Fiverr | Holds freelancer payment until work accepted | 3-7 days |
| Real Estate Closing | Title company holds funds until deed transfer | 30-60 days |
| Escrow.com | Domain/vehicle transfers | 1-10 days |

All of these share the same core backend pattern: a **state machine** managing funds between parties, with an arbiter for disputes. This project rebuilds that pattern on Solana.

---

## How It Works in Web2

```
┌─────────────────────────────────────────────────────────────────────────┐
│                       WEB2 ESCROW ARCHITECTURE                          │
│                                                                         │
│  Client App  ──HTTP──▶  REST API  ──SQL──▶  PostgreSQL                 │
│  (React/Vue)            (Express)           ┌───────────────────────┐   │
│                                             │ escrows               │   │
│  Auth: JWT Bearer       Input Validation    │  id          BIGINT   │   │
│  ────────────────       ─────────────────   │  buyer_id    UUID     │   │
│  { "email": "...",      { amount > 0,       │  seller_id   UUID     │   │
│    "password": "..." }     buyer ≠ seller } │  arbiter_id  UUID     │   │
│                                             │  amount      BIGINT   │   │
│                         Payment Rails       │  status      VARCHAR  │   │
│                         ─────────────       │  description TEXT     │   │
│                         Stripe / ACH /      │  created_at  TIMESTAMP│   │
│                         Wire Transfer       └───────────────────────┘   │
│                              │                                          │
│                              ▼                                          │
│                    ┌──────────────────┐       ┌──────────────────┐      │
│                    │  Holding Account │       │  Trust Layer     │      │
│                    │  (Stripe Balance)│◄──────│  (Your Company)  │      │
│                    └────────┬─────────┘       └──────────────────┘      │
│                             │ Release / Refund                          │
│                             ▼                                           │
│                    Seller or Buyer bank account                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Web2 Stack

| Layer | Technology | Role |
|---|---|---|
| **Frontend** | React / Vue / Mobile | User interface |
| **API** | Express.js / Django / Rails | REST endpoints, middleware |
| **Auth** | JWT / OAuth 2.0 / Passport | Identity verification |
| **Database** | PostgreSQL / MySQL | State storage (escrow records) |
| **Payment** | Stripe / ACH / Wire | Actual money movement |
| **Trust** | The company itself | Holds funds, enforces rules |
| **Disputes** | Support team + CRM | Human-reviewed, takes days/weeks |
| **Monitoring** | Datadog / Sentry | Observability |

### Core Weaknesses

1. **Counterparty risk** — the escrow company can be hacked, go bankrupt, or defraud users
2. **Slow settlement** — bank transfers take 1-10 business days
3. **Geographic restrictions** — not all payment methods work cross-border
4. **High fees** — payment processor + platform fees (2-5% typical)
5. **Opacity** — users must trust the platform's internal database state
6. **Single point of failure** — one server crash can lock everyone's funds

---

## How It Works on Solana

```
┌─────────────────────────────────────────────────────────────────────────┐
│                      SOLANA ESCROW ARCHITECTURE                         │
│                                                                         │
│  Client (CLI/dApp) ──sign tx──▶ Escrow Program (Rust/Anchor)           │
│                                         │                               │
│  Auth: Ed25519 Signature       ┌────────┴────────┐                      │
│  ────────────────────          │                  │                     │
│  Private key signs tx.         ▼                  ▼                     │
│  Runtime verifies before   EscrowAccount PDA   VaultAccount PDA         │
│  program even executes.    [buyer, seller,     [holds actual SOL]       │
│                             arbiter, state]          │                  │
│                                                      │                  │
│  State transitions:                                  ▼                  │
│  Active → Funded → Released              Seller's Wallet                │
│                  → Refunded              (instant settlement)           │
│                  → Disputed → Released/Refunded                         │
│                                                                         │
│  After settlement:                                                      │
│  closeEscrow() → both PDAs deleted, rent reclaimed                      │
└─────────────────────────────────────────────────────────────────────────┘
```

### Layer-by-Layer Comparison

| Web2 Layer | Solana Equivalent | Key Difference |
|---|---|---|
| **REST API** | Program instructions (`initialize_escrow`, `deposit`, etc.) | Instructions are tx-level, not HTTP requests |
| **PostgreSQL** | `EscrowAccount` PDA (335 bytes, Borsh-serialized) | Data replicated across thousands of validators |
| **JWT / OAuth** | Ed25519 signer verification + PDA seeds | Cryptographic — unforgeable, no middleware needed |
| **Stripe / ACH** | `system_program::transfer` CPI | Atomic, sub-second, ~$0.00025 per tx |
| **Trust layer** | The program itself | No company — math IS the escrow |
| **Support team** | `arbiter` pubkey with on-chain authority | Arbiter is a wallet, not a department |
| **DB cleanup** | `close_escrow` instruction (rent reclamation) | Recovers ~0.004 SOL per settled escrow |

### Solana Advantages

| Concern | Web2 | Solana |
|---|---|---|
| **Trust** | Trust the company | Trust the code (verified on-chain) |
| **Settlement** | 1-10 business days | ~400ms (1.5 slots) |
| **Access** | KYC, bank account required | Any wallet, anywhere |
| **Cost** | 2-5% platform + processor fees | ~$0.00025 per transaction |
| **Auditability** | Private DB, requires subpoena | Public blockchain, anyone can verify |
| **Uptime** | 99.9% SLA (8.7h downtime/year) | Decentralized — no single point of failure |
| **Immutability** | Company can alter records | Program is deployed, verifiable, auditable |

---

## Architecture Deep Dive

### PDAs vs. Database Rows

In Web2, each escrow is a row with a server-assigned UUID. The application layer mediates all access.

```sql
-- Web2: Server generates ID, writes to centralized DB
INSERT INTO escrows (buyer_id, seller_id, amount, status)
VALUES ('uuid-1', 'uuid-2', 50000000, 'active')
RETURNING id;

-- Lookup requires DB connection + auth middleware
SELECT * FROM escrows WHERE id = $1 AND buyer_id = $2;
```

In Solana, each escrow is a **Program Derived Address (PDA)** — an account whose address is deterministically derived from public inputs:

```rust
// Solana: Address is computed from seed material — no DB lookup needed
EscrowAccount = PDA(["escrow", buyer_pubkey, escrow_id_bytes])
VaultAccount  = PDA(["vault",  buyer_pubkey, escrow_id_bytes])
```

```typescript
// Client-side: anyone can compute the PDA address independently
const [escrowPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("escrow"), buyerKey.toBuffer(), idBytes],
    PROGRAM_ID
);
```

**Why this matters:**
- No lookup table needed — any client can derive the address from known inputs
- No collisions — each (buyer, escrow_id) pair maps to a unique, deterministic address
- Ownership is enforced by the runtime — only the escrow program can modify these accounts

### Authorization: Middleware vs. Runtime

**Web2** — authorization is application-layer middleware. A bug in your auth code means anyone can access anything:

```javascript
// Express.js: if you forget this middleware, the route is open
app.post('/escrows/:id/release', authenticate, (req, res) => {
    if (req.user.id !== escrow.buyer_id && req.user.role !== 'arbiter') {
        return res.status(403).json({ error: 'Forbidden' });
    }
    // ... release logic
});
```

**Solana** — authorization is enforced by the runtime before your code executes. The `Signer` constraint guarantees a valid Ed25519 signature:

```rust
// Solana: The runtime has already verified the signature before
// this code runs. No middleware bugs possible.
pub caller: Signer<'info>,  // ← runtime-enforced

require!(
    caller == escrow.buyer || caller == escrow.arbiter,
    EscrowError::Unauthorized
);
```

The difference: Solana's runtime-level signature verification runs in a sandboxed environment where the binary can't be bypassed. In Web2, a misconfigured reverse proxy, missing middleware, or debug route can expose privileged operations.

### The Vault: Real Account vs. Database Record

**Web2**: "Holding funds" means a record in Stripe's database says `amount=$50, status=held`. The money is actually in Stripe's bank account. If Stripe is breached, goes down, or changes their API — you have a problem.

**Solana**: The vault is a real on-chain account with actual SOL in its lamport balance. It's owned by your program, and the Solana runtime enforces that no other code can debit it:

```rust
// Only this program can execute this — enforced at the VM level
ctx.accounts.vault.sub_lamports(escrow.amount)?;
ctx.accounts.seller.add_lamports(escrow.amount)?;
```

### Rent Reclamation: The `close_escrow` Pattern

On Solana, every account must maintain a minimum lamport balance (rent exemption). For our escrow:

| Account | Size | Rent Cost |
|---|---|---|
| `EscrowAccount` | 335 bytes | ~0.0032 SOL |
| `VaultAccount` | 8 bytes | ~0.001 SOL |
| **Per escrow** | | **~0.004 SOL** |

Without cleanup, 1,000 escrows = ~4 SOL permanently locked in rent.

The `close_escrow` instruction solves this by closing settled (Released/Refunded) escrow accounts and returning the rent to the original buyer. This is the on-chain equivalent of database garbage collection.

```rust
// Anchor's `close` constraint handles the heavy lifting:
#[account(
    mut,
    close = buyer,                  // Transfer all lamports to buyer
    seeds = [...], bump = ...,
    has_one = buyer @ EscrowError::Unauthorized,
)]
pub escrow: Account<'info, EscrowAccount>,
```

In Web2, you'd archive old records to cold storage. On Solana, you close accounts and recover their rent — a critical production pattern.

---

## Design Decisions

### 1. Why Anchor over Native Rust SDK?

| Factor | Anchor | Native SDK |
|---|---|---|
| Account serialization | Automatic (Borsh + discriminator) | Manual |
| PDA validation | Declarative `seeds` + `bump` | Manual `create_program_address` |
| Error handling | `#[error_code]` + rich messages | Raw `ProgramError` |
| IDL generation | Automatic | None |
| Client codegen | TypeScript types from IDL | Manual |
| Security | Built-in discriminator checks, owner checks | Must implement manually |

For a backend-replacement demo targeting traditional developers, Anchor's declarative syntax is closer to how Web2 developers think about validation. The `#[derive(Accounts)]` macro is analogous to defining route middleware + DB schema constraints in a single place.

### 2. Why Separate Vault Account?

The vault could live as lamports on the EscrowAccount itself (common in simple programs). I chose a separate VaultAccount PDA to:

1. **Separation of concerns** — metadata (EscrowAccount) vs. funds (VaultAccount), mirroring the Web2 pattern of "database record" vs. "bank account"
2. **Clearer accounting** — the vault's lamport balance IS the escrowed amount (no confusion with rent)
3. **Extensibility** — in production, you'd swap the vault for an SPL Token account without changing the escrow metadata structure
4. **Teaching value** — demonstrates the Solana pattern of "multiple PDAs working together"

### 3. Why Arbiter as a Simple Pubkey?

A production system might use a multi-sig (e.g., Squads protocol) or an oracle for automated dispute resolution. I kept it as a single pubkey to:
- Focus on the core state machine (the bounty's primary requirement)
- Show the authorization pattern clearly
- Keep the account size predictable

### 4. Module Structure

```
instructions/
├── initialize.rs    # Create escrow + vault PDAs
├── deposit.rs       # Fund the vault with SOL
├── release.rs       # Pay the seller
├── refund.rs        # Return funds to buyer
├── dispute.rs       # Flag for arbiter resolution
└── close.rs         # Reclaim rent after settlement
```

Each instruction file contains:
1. **Web2 comparison** (doc comment) — what this replaces in a traditional system
2. **Handler function** — the actual instruction logic
3. **Accounts context** — declarative account validation (like middleware)

This mirrors the common pattern of `routes/` + `controllers/` + `middleware/` in Express.js.

---

## State Machine

```
                    initialize_escrow()
                           │
                           ▼
                      ┌────────┐
                      │ ACTIVE │  ← Escrow created, no funds yet
                      └────┬───┘    (Web2: "pending_payment")
                           │
                           │ deposit() [buyer signs]
                           ▼
                      ┌────────┐
                      │ FUNDED │  ← SOL locked in vault PDA
                      └──┬──┬──┘    (Web2: "in_escrow")
                         │  │
         release()       │  │  dispute()
    [buyer / arbiter]    │  │  [buyer / seller]
                         │  │
              ┌──────────┘  └──────────────┐
              │                            │
              ▼                            ▼
        ┌──────────┐                ┌──────────┐
        │ RELEASED │                │ DISPUTED │  (Web2: "under_review")
        └─────┬────┘                └────┬─────┘
              │                          │
              │        release() or refund()
              │        [arbiter resolves]
              │                          │
              │          ┌───────────────┤
              │          │               │
              │          ▼               ▼
              │    ┌──────────┐    ┌──────────┐
              │    │ RELEASED │    │ REFUNDED │
              │    └─────┬────┘    └─────┬────┘
              │          │               │
              ▼          ▼               ▼
        ┌─────────────────────────────────────┐
        │         close_escrow()              │
        │   Both PDAs closed, rent reclaimed  │
        │   (Web2: "archived" / DB cleanup)   │
        └─────────────────────────────────────┘
```

---

## Account Model

### EscrowAccount (data storage — 335 bytes)

| Field | Type | Size | Web2 Column |
|---|---|---|---|
| `discriminator` | `[u8; 8]` | 8 | (Anchor internal — type safety) |
| `escrow_id` | `u64` | 8 | `id BIGINT PRIMARY KEY` |
| `buyer` | `Pubkey` | 32 | `buyer_id UUID REFERENCES users` |
| `seller` | `Pubkey` | 32 | `seller_id UUID REFERENCES users` |
| `arbiter` | `Pubkey` | 32 | `arbiter_id UUID REFERENCES users` |
| `amount` | `u64` | 8 | `amount BIGINT NOT NULL` |
| `description` | `String` | 4 + 200 | `description TEXT CHECK(len<=200)` |
| `state` | `EscrowState` | 1 | `status VARCHAR(20)` |
| `created_at` | `i64` | 8 | `created_at TIMESTAMP` |
| `escrow_bump` | `u8` | 1 | — (PDA mechanics, no Web2 parallel) |
| `vault_bump` | `u8` | 1 | — (PDA mechanics) |

### VaultAccount (SOL holder — 8 bytes)

| Field | Type | Size | Web2 Equivalent |
|---|---|---|---|
| `discriminator` | `[u8; 8]` | 8 | — |
| **SOL balance** | *lamports* | — | Stripe holding account balance |

---

## Instruction Reference

| # | Instruction | Web2 Endpoint | Who Can Call | State Before | State After |
|---|---|---|---|---|---|
| 1 | `initialize_escrow` | `POST /escrows` | Anyone (becomes buyer) | — | Active |
| 2 | `deposit` | `PUT /escrows/:id/deposit` | Buyer only | Active | Funded |
| 3 | `release` | `POST /escrows/:id/release` | Buyer or Arbiter | Funded/Disputed | Released |
| 4 | `refund` | `POST /escrows/:id/refund` | Seller or Arbiter | Funded/Disputed | Refunded |
| 5 | `dispute` | `POST /escrows/:id/dispute` | Buyer or Seller | Funded | Disputed |
| 6 | `close_escrow` | `DELETE /escrows/:id` | Buyer only | Released/Refunded | (deleted) |

---

## Error Handling

Custom errors with clear messages, mapping to familiar HTTP status codes:

| Error Code | Message | HTTP Analog |
|---|---|---|
| `DescriptionTooLong` | Description exceeds 200 character limit | 400 Bad Request |
| `InvalidAmount` | Escrow amount must be greater than zero | 400 Bad Request |
| `BuyerCannotBeSeller` | Buyer and seller must be different accounts | 400 Bad Request |
| `InvalidArbiter` | Arbiter must be different from buyer and seller | 400 Bad Request |
| `Unauthorized` | Caller is not authorized to perform this action | 403 Forbidden |
| `InvalidState` | Escrow is not in the required state for this operation | 409 Conflict |
| `InvalidSeller` | Provided seller account does not match escrow record | 400 Bad Request |
| `InvalidBuyer` | Provided buyer account does not match escrow record | 400 Bad Request |
| `EscrowNotSettled` | Escrow must be in Released or Refunded state to close | 409 Conflict |

---

## Tradeoffs & Constraints

### ✅ What Solana Does Better

| Concern | Web2 | Solana |
|---|---|---|
| **Trust** | Trust the company | Trust the code (math > middlemen) |
| **Settlement** | 1-3 business days | ~400ms |
| **Global access** | KYC, bank restrictions | Open to any wallet holder |
| **Auditability** | Private DB, FOIA needed | Public blockchain |
| **Censorship** | Company can freeze funds | Program can't be modified after deploy |
| **Cost** | 2-5% platform fees | ~$0.00025/tx |
| **Atomicity** | Multi-step processes, partial failures | All-or-nothing transactions |

### ⚠️ What Web2 Does Better

| Concern | Web2 | Solana |
|---|---|---|
| **Legal recourse** | Courts, chargebacks, consumer protection | Users bear full responsibility |
| **Fiat currency** | Native USD/EUR/GBP | SOL + stablecoins only |
| **User experience** | Email + password, familiar UI | Wallet required, key management |
| **Error recovery** | Support team can fix mistakes | Immutable — bugs are permanent |
| **Complex logic** | Unlimited compute, any language | 200k compute units per tx |
| **Off-chain data** | Unlimited storage, rich media | 10KB account size limit |
| **Privacy** | Private by default | Public by default |

### 🔧 Technical Constraints on Solana

1. **Account size is fixed at creation** — can't grow the `description` field later
2. **Compute budget** — complex arbiter logic must stay within 200k CU per tx
3. **No off-chain triggers** — can't auto-release after 30 days without a crank/oracle
4. **SOL only in this implementation** — SPL token support requires `anchor-spl`
5. **Rent requirement** — accounts must maintain minimum balance (solved by `close_escrow`)
6. **No dynamic dispatch** — only pre-declared CPI targets

### 🔮 Production Extensions

To take this to production:

| Extension | Approach |
|---|---|
| **SPL tokens** | Use `anchor-spl` for USDC/USDT escrow |
| **Timelock** | Store `deadline: i64` in EscrowAccount, add auto-cancel via Clockwork |
| **Multi-sig arbiter** | Integrate Squads protocol for 2-of-3 committee |
| **Privacy** | Store only a Merkle root on-chain, full data on Arweave |
| **Fee collection** | Add a protocol fee (0.1%) deducted on release |
| **Partial release** | Split amount into milestones |
| **Event indexing** | Use Helius webhooks to index `EscrowCreated`, `EscrowReleased` events |

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
# Shorthand (optional)
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

# 4. Close settled escrow and reclaim rent
CLUSTER=devnet escrow close --id 42

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
# Set cluster to Localnet for testing
# (change [provider] cluster in Anchor.toml, or override):
anchor test

# Expected output:
#   solana-escrow-engine
#     ✔ 1. Initializes an escrow (buyer creates PDA accounts)
#     ✔ 2. Deposits SOL into vault (buyer funds the escrow)
#     ✔ 3. Rejects dispute from an outsider (access control)
#     ✔ 4. Rejects release from an outsider (access control)
#     ✔ 5. Rejects release with wrong seller address
#     ✔ 6. Buyer opens a dispute (state: Funded → Disputed)
#     ✔ 7. Arbiter releases funds to seller (dispute resolution)
#     ✔ 8. Rejects release on already released escrow (double-release guard)
#     ✔ 9. Rejects double-deposit on already funded escrow (state guard)
#     ✔ 10. Rejects dispute on Active escrow (not yet funded)
#     ✔ 11. Rejects zero-amount escrow (input validation)
#     ✔ 12. Rejects escrow where buyer == seller
#     ✔ 13. Seller voluntarily refunds buyer (happy path refund)
#     ✔ 14. Buyer releases funds directly to seller (happy path)
#     ✔ 15. Closes released escrow and reclaims rent to buyer
#     ✔ 16. Closes refunded escrow and reclaims rent to buyer
#     ✔ 17. Rejects close_escrow on funded (unsettled) escrow
#
#   17 passing
```

### Test Coverage Matrix

| # | Category | Test | What It Validates |
|---|---|---|---|
| 1 | Core lifecycle | Initialize escrow | PDA created, all fields correct, timestamp set |
| 2 | Core lifecycle | Deposit SOL | Vault balance increase, state → Funded |
| 3 | Access control | Outsider dispute rejected | Only buyer/seller can dispute |
| 4 | Access control | Outsider release rejected | Only buyer/arbiter can release |
| 5 | Access control | Wrong seller rejected | Seller pubkey must match escrow record |
| 6 | Dispute flow | Buyer opens dispute | State → Disputed |
| 7 | Dispute flow | Arbiter resolves (release) | Seller receives SOL, state → Released |
| 8 | State guard | Double-release rejected | Can't release twice (Released is terminal) |
| 9 | State guard | Double-deposit rejected | Can't deposit into already Funded escrow |
| 10 | State guard | Dispute on unfunded rejected | Must be Funded to dispute |
| 11 | Input validation | Zero amount rejected | Amount > 0 enforced |
| 12 | Input validation | Buyer == seller rejected | Distinct parties required |
| 13 | Happy path | Seller refunds buyer | Voluntary refund, buyer receives SOL |
| 14 | Happy path | Buyer releases directly | No dispute needed, seller paid |
| 15 | Rent reclamation | Close released escrow | Accounts deleted, rent reclaimed |
| 16 | Rent reclamation | Close refunded escrow | Both terminal states support close |
| 17 | Rent reclamation | Close funded escrow rejected | Can't close while funds are locked |

---

## Project Structure

```
solana-escrow-engine/
├── programs/
│   └── solana-escrow-engine/
│       └── src/
│           ├── lib.rs              # Program entry — thin handlers
│           ├── state.rs            # EscrowAccount, VaultAccount, EscrowState
│           ├── errors.rs           # Custom error codes (9 variants)
│           ├── events.rs           # On-chain event structs (5 events)
│           └── instructions/       # One file per instruction
│               ├── mod.rs          # Module re-exports
│               ├── initialize.rs   # Create escrow + vault PDAs
│               ├── deposit.rs      # Fund the vault with SOL
│               ├── release.rs      # Pay the seller
│               ├── refund.rs       # Return funds to buyer
│               ├── dispute.rs      # Flag for arbiter resolution
│               └── close.rs        # Reclaim rent after settlement
├── tests/
│   └── solana-escrow-engine.ts     # 17 integration tests
├── client/
│   ├── cli.ts                      # TypeScript CLI (7 commands)
│   └── devnet_demo.ts              # Full lifecycle demo script
├── target/
│   ├── idl/solana_escrow_engine.json   # Auto-generated IDL
│   └── types/solana_escrow_engine.ts   # Auto-generated TypeScript types
├── Anchor.toml                     # Anchor config
├── Cargo.toml                      # Rust workspace
├── package.json                    # Node dependencies
└── README.md                       # This file
```

### Why This Structure?

| Web2 Analogy | Solana File | Purpose |
|---|---|---|
| `routes/` | `instructions/` | Each instruction = one route handler |
| `models/` | `state.rs` | Database schema = account struct |
| `middleware/errors.ts` | `errors.rs` | HTTP error codes = Anchor error codes |
| `services/events.ts` | `events.rs` | Webhook events = on-chain events |
| `app.ts` (entry point) | `lib.rs` | Routes registered + handlers delegated |

---

## Devnet Transaction Log

> All transactions executed on Solana Devnet — publicly verifiable.

| # | Action | Transaction | Explorer |
|---|---|---|---|
| 1 | **Program Deploy** | `5DLg4Nan...` | [View](https://explorer.solana.com/tx/5DLg4NanzsV5NFtWCkgzvFWjGz2otxKHztZbE7waAttvaX1csJrELoKKWRwhhFMZ5B35TPGeHactEGTLL64jNNek?cluster=devnet) |
| 2 | **initializeEscrow** | `5hikQTe8...` | [View](https://explorer.solana.com/tx/5hikQTe8ubhYSRDKTMhsDTrpSKdJSdzgqXmraXEcSWwgKnYd1pjw9dhyFNp8rvVsiY1EBjfJ2hTPKYucvMBFjtMb?cluster=devnet) |
| 3 | **deposit** | `241E9awr...` | [View](https://explorer.solana.com/tx/241E9awrvjGzpx97AUsdo6GRBLMCdoHMKhb3UwPjkCGETw9XYukaeKzx6dxQLiWza727fEVHnG735ceArJKsqbxm?cluster=devnet) |
| 4 | **release** | `5DfWyGGs...` | [View](https://explorer.solana.com/tx/5DfWyGGsPU2YvDRsLSTTMkF7vwTGLrnN1ZG6bNxETs2UwAnirensHUHQoWsJePmmW5PCtLGQAU8rz5PMC9UCxFnp?cluster=devnet) |
| 5 | **closeEscrow** | `3tn1j3cC...` | [View](https://explorer.solana.com/tx/3tn1j3cCinqTYY7ee7RWQreQkH9RxxPXAsG1s9PWnFhPDb9eD3ruGngSwqxL6Y7DVZMnUuV36Kd6VcMEXLDnMhUP?cluster=devnet) |

**Full lifecycle demonstrated on-chain:**
1. `initializeEscrow` — created EscrowAccount + VaultAccount PDAs
2. `deposit` — locked 0.01 SOL in vault PDA  
3. `release` — buyer approved, 0.01 SOL transferred to seller instantly
4. `closeEscrow` — settled escrow cleaned up, **0.004 SOL rent reclaimed**

**Program address:** [`9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF`](https://explorer.solana.com/address/9d2ZuC4PjjPDdTYuonAvZ3U5yQkmEKPesVVhp9LRtDF?cluster=devnet)

---

## License

MIT — free to use, fork, and learn from.
