# Milestone Escrow Contract

A Soroban smart contract on Stellar that enables trustless milestone-based escrow between a client, freelancer, and arbiter.

## Overview

This contract allows a client to fund a job broken into milestones. Funds are locked in the contract and released per milestone only when the client approves delivery. Disputes can be raised by either party and resolved by a designated arbiter.

## Trust Model

The escrow's protection depends on **four distinct roles** assigned at `initialize` time:

- **Admin** — the platform operator. Holds admin-only endpoints (whitelist management, pause/resume, admin overrides, upgrades). The admin is expected to be a neutral platform address, not the client or freelancer.
- **Client** — funds the job and approves milestone releases.
- **Freelancer** — delivers milestones and receives payouts.
- **Arbiter** — a neutral third party that resolves disputes via `resolve_dispute` and `apply_dispute_arbitration_split`.

`initialize` enforces party distinctness: it rejects any collision among `admin`, `client`, `freelancer`, and `arbiter` with `Error::InvalidParties`. This prevents the client from resolving their own disputes (arbiter == client), the freelancer from ruling in their own favour (arbiter == freelancer), or the client from holding every admin override (admin == client). The frontend must pass four genuinely distinct addresses — it must not reuse the connected wallet as both admin and client.

## Contract Functions

The contract exposes **94 public functions** grouped by category below.

### Core Escrow (Initialization & Funding)

| Function | Caller | Description |
|---|---|---|
| `initialize` | Anyone | Set up job with milestone amounts, spec hashes, 30-day dispute timeout, Upfront funding |
| `initialize_with_dispute_timeout` | Anyone | Set up job with a custom, non-zero dispute timeout |
| `initialize_with_funding_mode` | Anyone | Set up job with Upfront or PerMilestone funding mode |
| `fund` | Client | Deposit total amount into contract (Upfront mode) |
| `fund_milestone` | Client | Deposit one milestone (PerMilestone mode) |
| `get_funding_mode` | Anyone | View the current funding mode |
| `accept_job` | Freelancer | Freelancer accepts the job |
| `accept_arbiter` | Arbiter | Arbiter accepts the job |
| `withdraw_unaccepted` | Client | Reclaim funds if job not accepted within 7 days |
| `get_job` | Anyone | View current job state |
| `version` | Anyone | Get contract version |

### Milestone Lifecycle

| Function | Caller | Description |
|---|---|---|
| `mark_delivered` | Freelancer | Mark a milestone as delivered |
| `request_revision` | Client | Return a delivered milestone to Pending (max 3 revisions) |
| `get_revision_count` | Anyone | Get revision count for a milestone |
| `extend_milestone_deadline` | Client | Extend auto-release deadline for a delivered milestone |
| `claim_auto_release` | Freelancer | Auto-release a delivered milestone after deadline |
| `time_until_auto_release` | Anyone | Time remaining until auto-release |
| `approve_partial` | Client | Partially release funds for a milestone |
| `approve_milestone` | Client | Release funds for a delivered milestone |
| `withdraw` | Freelancer/Client | Withdraw released funds or refunds |

### Disputes & Arbitration

| Function | Caller | Description |
|---|---|---|
| `raise_dispute` | Client or Freelancer | Freeze a milestone for arbitration |
| `escalate_dispute` | Client or Freelancer | After timeout, restore milestone to pre-dispute status |
| `resolve_dispute` | Arbiter | Release to freelancer or refund to client |
| `dispute_arbitration_split` | Arbiter | Propose a split between client/freelancer |
| `apply_dispute_arbitration_split` | Arbiter | Apply the proposed arbitration split |

### Cancellation

| Function | Caller | Description |
|---|---|---|
| `cancel_escrow` | Client or Freelancer | Initiate escrow cancellation |
| `revoke_cancel_approval` | Client or Freelancer | Revoke cancellation approval |
| `admin_override_cancel_release` | Admin | Admin override: release to freelancer |
| `admin_override_cancel_refund` | Admin | Admin override: refund to client |

### Emergency Pause

| Function | Caller | Description |
|---|---|---|
| `emergency_pause` | Client & Freelancer | Jointly pause the contract |
| `emergency_unpause` | Admin | Admin unpause the contract |
| `emergency_unpause_parties` | Client & Freelancer | Jointly lift a party-initiated pause |
| `emergency_pause_admin_override` | Admin | Admin override pause state |
| `is_emergency_paused` | Anyone | Check if contract is paused |
| `emergency_pause_split_refund` | Admin/Client/Freelancer | Compute split while paused (read-only) |
| `emergency_pause_claim_refund` | Admin | Settle emergency refund (claim) |
| `emergency_pause_allocation` | Admin/Client/Freelancer | Allocate frozen balance to parties |

### Admin & Upgrades

| Function | Caller | Description |
|---|---|---|
| `transfer_admin` | Admin | Transfer admin role |
| `add_whitelisted_token` | Admin | Add token to whitelist |
| `remove_whitelisted_token` | Admin | Remove token from whitelist |
| `is_token_whitelisted` | Anyone | Check if token is whitelisted |
| `get_whitelisted_tokens` | Anyone | List all whitelisted tokens |
| `upgrade` | Admin | Upgrade contract WASM |
| `migrate_cold_config` | Admin | Migrate config out of instance storage |
| `admin_pause_escrow` | Admin | Admin pause (blocks user endpoints) |
| `admin_resume_escrow` | Admin | Admin resume |
| `propose_admin_transfer` | Admin | Propose multisig admin transfer |
| `execute_admin_transfer` | Multisig | Execute proposed admin transfer |
| `cancel_admin_transfer_proposal` | Admin | Cancel pending admin transfer |
| `get_pending_admin_transfer` | Anyone | View pending admin transfer |

### Platform Fees

| Function | Caller | Description |
|---|---|---|
| `set_platform_fee_allocation` | Admin | Set fee split (client/freelancer/treasury) |
| `lock_platform_fee_allocation` | Admin | Lock fee allocation |
| `pf_alloc_admin_override` | Admin | Admin override for fee allocation |
| `get_platform_fee_allocation` | Anyone | View current fee allocation |
| `calculate_platform_fee_split` | Anyone | Calculate fee split for an amount |

### Payment Streaming

| Function | Caller | Description |
|---|---|---|
| `payment_streaming_milestones` | Admin | Configure streaming for milestones |
| `payment_streaming_consent` | Client/Freelancer | Consent to streaming config |

### Time Extensions

| Function | Caller | Description |
|---|---|---|
| `milestone_time_extensions` | Admin | Configure time extensions |
| `time_extensions_consent` | Client/Freelancer | Consent to time extensions |

### Multisig

| Function | Caller | Description |
|---|---|---|
| `multisig_approval_init` | Admin | Initialize multisig signers & threshold |
| `multisig_approve` | Signer | Approve a multisig proposal |
| `is_multisig_approved` | Anyone | Check if proposal is approved |
| `multisig_lock` | Admin | Lock multisig (require admin intervention) |
| `is_multisig_locked` | Anyone | Check if multisig is locked |
| `multisig_admin_override_release` | Admin | Override locked multisig: release |
| `multisig_admin_override_refund` | Admin | Override locked multisig: refund |
| `multisig_split_refund` | Signers | Split refund via multisig |

### Interest Yield

| Function | Caller | Description |
|---|---|---|
| `escrow_interest_yield` | Anyone | Estimate interest yield |
| `set_escrow_interest_yield` | Client/Freelancer | Configure yield share |
| `set_interest_yield_consent` | Client/Freelancer | Consent to yield config |
| `lock_escrow_interest_yield` | Admin | Lock yield config |
| `unlock_escrow_interest_yield` | Admin | Unlock yield config |
| `is_escrow_interest_yield_locked` | Anyone | Check if yield is locked |
| `interest_yield_split_refund` | Admin | Split refund with yield |
| `get_escrow_interest_yield` | Anyone | View yield state |
| `admin_set_yield_rate` | Admin | Set annual yield rate (bps) |
| `admin_accrue_yield` | Admin | Accrue yield to admin |

### Tax Withholding

| Function | Caller | Description |
|---|---|---|
| `tax_withholding_deductions` | Admin | Compute tax withholding |
| `tax_withholding_split_refund` | Admin | Split refund with tax |
| `admin_override_tax_release` | Admin | Override tax: release |
| `admin_override_tax_refund` | Admin | Override tax: refund |
| `admin_override_tax_split_refund` | Admin | Override tax: split refund |
| `get_yield_info` | Anyone | View yield rate, accrued, paused |
| `admin_tax_withholding_deductions` | Admin | Admin compute tax withholding |

### Utilities

| Function | Caller | Description |
|---|---|---|
| `sweep_excess` | Client or Admin | Recover surplus tokens |
| `get_milestone_spec_hash` | Anyone | Get spec hash for milestone |
| `get_reputation` | Anyone | Get address reputation |
| `compute_ratio_allocation` | Anyone | Compute ratio allocation (largest remainder) |

## Milestone States

Pending → Delivered → Released

↓

Disputed → Released (arbiter favors freelancer)

→ Refunded (arbiter favors client)

The full state machine, including every entry point, its required signers and the escrow-level locks, is specified in [`contracts/milestone-escrow/SPEC.md`](contracts/milestone-escrow/SPEC.md).

## Security

Roles, trust assumptions, admin powers and known risks are described in [`THREAT_MODEL.md`](THREAT_MODEL.md).

## Prerequisites

- https://rustup.rs/ (the toolchain is pinned to 1.96.0 by `rust-toolchain.toml`)
- https://developers.stellar.org/docs/smart-contracts/getting-started/setup (stellar-cli 28.0.0)
- wasm32 target: `rustup target add wasm32v1-none`
- Docker, for reproducible builds

## Build

For development, from the repository root:

```bash
stellar contract build --manifest-path contracts/milestone-escrow/Cargo.toml
```

Anything that will be deployed must come from the reproducible build instead:

```bash
scripts/build-reproducible.sh
```

It compiles a clean export of `HEAD` (or of the commit given as the first
argument) inside a pinned container (`build/reproducible.Dockerfile`: Rust
1.96.0 by image digest, stellar-cli 28.0.0 by release checksum, linux/amd64)
and writes `dist/milestone_escrow.wasm`, its SHA-256 and `dist/build-info.json`.
The same commit gives the same hash on any machine. CI runs the same script on
every commit and publishes the hash in the job summary and the wasm as an
artifact.

## Test

From the repository root:

```bash
cargo test -p milestone-escrow
```

## Project Status

- Mainnet release gates are tracked in the [mainnet readiness checklist](MAINNET.md)
- The admin authorization direction and migration decision are recorded in [ADMIN_MULTISIG_DECISION.md](ADMIN_MULTISIG_DECISION.md)
- Contract implementation completed in `contracts/milestone-escrow`
- Includes milestone delivery, approval, dispute, and arbitration flows
- Initialize now rejects empty milestone lists, zero/negative amounts, zero addresses, zero auto-release windows, and **party collisions** (`admin == client`, `admin == freelancer`, `arbiter == client`, `arbiter == freelancer`, `client == freelancer`) with explicit contract errors
- `raise_dispute` now performs zero-address check, `require_auth`, and identity validation **before** writing the re-entrancy lock (issue #566), matching the authorization pattern used by `fund`, `approve_milestone`, and `resolve_dispute`; includes the five precondition guard assertions present in all other mutating functions
- Disputes may only be raised on delivered or partially released milestones. The default dispute timeout is 30 days; `initialize_with_dispute_timeout` allows a custom non-zero timeout. If neither party gets an arbitration decision by that deadline, either party may call `escalate_dispute` to return the milestone to its pre-dispute status; a restored delivered milestone gets a fresh auto-release window
- `lock_escrow_interest_yield` now rejects unauthorized callers with `Unauthorized` and an illegal source state with `InvalidStatus` **before** it reads or writes the ledger entry (issue #464); authorization runs first via `require_admin_from_instance`, so a non-admin cannot probe whether an escrow is locked, and an already-locked configuration is refused rather than silently re-locked
- Contract tests and snapshots are provided under `contracts/milestone-escrow/test_snapshots`

## Deploy (Testnet)

```bash
scripts/build-reproducible.sh
stellar contract deploy \
  --wasm dist/milestone_escrow.wasm \
  --network testnet \
  --source <your-account>
```

Then record the contract ID, `source_commit` and `wasm_sha256` from
`dist/build-info.json` in `deployments/<network>.json`, and the hash in
`CHANGELOG.md`.

After upgrading an existing deployment to this version, the admin should call
`migrate_cold_config` once to move the whitelist, multisig, yield and fee
configuration out of instance storage (issue #772). The contract keeps working
before that call; the migration only makes every later call cheaper.

## Verifying the deployed contract

Anyone can check which source a deployed contract was built from:

1. Read the installed wasm hash:

   ```bash
   stellar contract fetch --id <contract-id> --network testnet --out-file deployed.wasm
   sha256sum deployed.wasm
   ```

   (`stellar contract info` and the explorer show the same hash.)
2. Look up `source_commit` for that contract in `deployments/<network>.json`.
3. Rebuild that commit and compare:

   ```bash
   scripts/build-reproducible.sh <source_commit>
   cat dist/milestone_escrow.wasm.sha256
   ```

The two hashes must be identical. If they differ, the deployed code is not the
reviewed source at that commit.

## License

MIT

## Deployed Contract

| Network | Contract ID |
|---|---|
| Testnet | `CDD5WKK3WT3QVKXMXTJNDIXE4T73FK6GGXDSD6UTJAH6YYZU52SQ4MUH` |

Deployment records, including the installed wasm hash, are in
[`deployments/`](deployments/). The current testnet deployment predates
reproducible builds, so its source commit is unknown.

Explorer: `https://stellar.expert/explorer/testnet/contract/CDD5WKK3WT3QVKXMXTJNDIXE4T73FK6GGXDSD6UTJAH6YYZU52SQ4MUH`
