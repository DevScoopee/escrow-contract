# TypeScript bindings (issue #744)

The escrow contract's ABI is published as generated TypeScript bindings so the
frontend and backend stop hand-writing ScVal encodings (e.g.
`{ type: "u64", value }` argument mappers) that silently break on any contract
signature change.

## How they are produced

The `typescript-bindings` job in `.github/workflows/ci.yml` runs for every
build on `main` (and every PR):

1. `stellar contract build` produces `milestone_escrow.wasm`.
2. `stellar contract id checksummed` derives the deploy-time contract address
   of that wasm on the chosen network (default `testnet`, overridable via the
   `bindings-network` input on `workflow_dispatch`).
3. `stellar contract bindings typescript --wasm ... --output-dir bindings/ts
   --contract-id ...` generates a typed `@goldii-locks/milestone-escrow-bindings`
   npm package under the repo's contract name.
4. The package is uploaded as the `milestone-escrow-bindings-ts-<sha>` run
   artifact.

On a `v*` tag, the `publish-bindings` job publishes the same package to GitHub
Packages (`https://npm.pkg.github.com`) with the tag version, e.g.
`@goldii-locks/milestone-escrow-bindings@1.2.3`. Main builds get
`0.0.0-main.<run>` artifact builds (not published).

> The contract id baked into the generated package is the checksum address of
> the built wasm. The escrow is deployed per-instance on Soroban, so consumers
> must pass the *deployed* contract id when constructing the client (shown
> below); the baked-in value only matters if you use the generated default.

## Consuming from escrow-frontend

```bash
# .npmrc (repo root):
@goldii-locks:registry=https://npm.pkg.github.com
//npm.pkg.github.com/:_authToken=${NPM_TOKEN}   # any GitHub PAT with read:packages
```

```ts
import { escrowContract, contract } from "@goldii-locks/milestone-escrow-bindings";
import { SorobanRpc, Networks } from "@stellar/js-xdr"; // existing soroban client stack

// Instead of hand-building { func: "initialize", args: vec([native(u64(...))]) }
// (see app/create/page.tsx), call the typed method:
const client = escrowContract(
  new Contract(network, rpc, account, deployedContractId),
);
await client.initialize(admin, clientAddr, freelancerAddr, arbiterAddr, token,
  autoReleaseSeconds, milestoneAmounts);
```

Reads are typed too: `await client.getJob()` returns the generated `Job`
structure — `milestones: Vec<Milestone>`, `status: MilestoneStatus`, etc. — so
decoding is no longer hand-rolled.

## Consuming from escrow-backend

The backend's build-tx mapper (`src/routes/jobs.ts`) and its ScVal decoding of
`get_job` / `get_whitelisted_tokens` should import the generated types and
`asValue`/`fromnative` helpers from the same package rather than hand-writing
`{ type: "u64", value }` encoders. The generated `MilestoneStatus` / `Job`
union types mirror the contract's `#[contracttype]` definitions exactly, which
is what the broken decoders were missing.

## Regenerating locally

```bash
stellar contract build --manifest-path contracts/milestone-escrow/Cargo.toml
CONTRACT_ID=$(stellar contract id checksummed \
  --wasm target/wasm32v1-none/release/milestone_escrow.wasm --network testnet | tail -n1)
stellar contract bindings typescript \
  --wasm target/wasm32v1-none/release/milestone_escrow.wasm \
  --output-dir bindings/ts \
  --contract-id "$CONTRACT_ID"
```

## Switching the clients over

This repo only *produces* the bindings. Track the client-side migration in the
client repos:

- escrow-frontend: replace the hand-written `initialize` encoding in
  `app/create/page.tsx` and all other raw-ScVal call sites with the package.
- escrow-backend: replace the build-tx mapper in `src/routes/jobs.ts` and the
  hand-rolled `get_job` / `get_whitelisted_tokens` decoders with the package.

A follow-up issue in each client repo should link back to this repo's
`typescript-bindings` CI job and pin the released version they migrate to.
