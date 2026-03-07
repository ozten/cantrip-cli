---
title: "feat: Add billing command with balance, history, and tiers subcommands"
type: feat
status: completed
date: 2026-03-07
---

# feat: Add billing command with balance, history, and tiers subcommands

## Overview

Add a `cantrip billing` command with three subcommands (`balance`, `history`, `tiers`) to cantrip-cli. The daemon already fully implements billing dispatch -- the CLI just needs to build the right JSON envelope and format the response. Also add corresponding MCP tools to mcp-server-cantrip.

## Problem Statement / Motivation

The cantrip daemon has a complete billing system (credit ledger, Stripe integration, tier config) but the CLI has zero billing awareness. Users cannot check their credit balance, view transaction history, or see available credit packs from the command line. The MCP server similarly lacks billing tools, meaning AI agents cannot query billing status.

## Proposed Solution

Follow the existing `Apikey`/`ApikeyAction` pattern in cantrip-cli to add a `Billing` command with `BillingAction` subcommand enum. Add billing-specific human output formatting in `output.rs`. Add three thin MCP tools in mcp-server-cantrip.

## Technical Approach

### Daemon Contract (confirmed from dispatch.rs:843-919)

The daemon expects command `"billing"` with the action as the first positional arg (defaults to `"balance"` if omitted). It requires a `team` flag (injected via auth). It does NOT use the `project` flag.

**Request → Response mappings:**

| CLI Command | JSON Envelope | Response Shape |
|---|---|---|
| `cantrip billing` | `{command: "billing", args: [], flags: {}}` | balance (daemon defaults) |
| `cantrip billing balance` | `{command: "billing", args: ["balance"], flags: {}}` | `{balance_credits, available_credits, reserved_credits, balance_microcredits, available_microcredits}` |
| `cantrip billing history` | `{command: "billing", args: ["history"], flags: {limit: "20"}}` | `{entries: [{id, amount_credits, balance_after_credits, entry_type, description, created_at}]}` |
| `cantrip billing history --limit 5` | `{command: "billing", args: ["history"], flags: {limit: "5"}}` | same as above |
| `cantrip billing tiers` | `{command: "billing", args: ["tiers"], flags: {}}` | `{tiers: [{tier, display_name, price_cents, credits}]}` |

### Key Design Decisions

1. **Default subcommand**: Use `Option<BillingAction>` in the Command enum, treat `None` as balance in `build_request`. This matches the daemon's own default behavior.

2. **Project flag**: Billing is account-level. The global project injection in `build_request` will include it if set, but the daemon ignores it -- no special handling needed.

3. **Custom human formatting**: The generic recursive formatter produces ugly output for billing data (shows microcredits, no alignment, no sign prefixes). Add billing-specific formatting in `output.rs` -- this is the right call for UX quality. The formatter will check the command string and dispatch to specialized formatters when matched.

4. **Tier sorting**: Sort tiers by `price_cents` ascending for consistent display (daemon returns them from a HashMap in arbitrary order).

5. **History limit default**: 20, matching the daemon's own default (dispatch.rs:882).

6. **Abundance framing** (per credit_ux.md): Balance output should include an operations estimate alongside raw credits (e.g., "~87 operations remaining").

## Changes by File

### 1. `src/cli/mod.rs` -- Add BillingAction enum and Billing command variant

```rust
// src/cli/mod.rs -- new enum
#[derive(Subcommand, Debug, Clone)]
pub enum BillingAction {
    /// Show credit balance (default)
    Balance,
    /// Show credit transaction history
    History {
        /// Maximum number of entries to show
        #[arg(long, default_value = "20")]
        limit: u32,
    },
    /// Show available credit packs and pricing
    Tiers,
}
```

Add to Command enum:
```rust
/// Manage billing and credits
Billing {
    #[command(subcommand)]
    action: Option<BillingAction>,
},
```

Using `Option<BillingAction>` so `cantrip billing` with no subcommand works (maps to balance).

### 2. `src/main.rs` -- Add billing arm in build_request()

```rust
// src/main.rs -- in build_request() match
Command::Billing { action } => match action.unwrap_or(BillingAction::Balance) {
    BillingAction::Balance => {
        ("billing".to_string(), vec!["balance".to_string()], flags)
    }
    BillingAction::History { limit } => {
        flags.insert("limit".to_string(), limit.to_string());
        ("billing".to_string(), vec!["history".to_string()], flags)
    }
    BillingAction::Tiers => {
        ("billing".to_string(), vec!["tiers".to_string()], flags)
    }
},
```

Import `BillingAction` at the top alongside existing action imports.

### 3. `src/output.rs` -- Add billing-specific human formatting

Add a function that checks the command and dispatches to billing formatters before falling through to the generic formatter.

**Balance output:**
```
Credit Balance
  Available:  435 credits (~87 operations remaining)
  Reserved:    15 credits (in-progress operations)
  Total:      450 credits
```

**History output:**
```
Credit History (last 20)
  +200.0  purchase  Starter pack               2026-03-07 14:30
   -15.0  usage     Analysis: ICP deep dive     2026-03-07 15:12
    -5.0  usage     LLM: Channel suggestions    2026-03-07 15:45
```

- Positive amounts prefixed with `+`, negative with `-` (already negative from daemon)
- Null descriptions rendered as empty string
- `created_at` truncated to `YYYY-MM-DD HH:MM`
- Empty entries array shows "No billing history yet."

**Tiers output:**
```
Credit Packs
  Starter    $19     200 credits
  Growth     $49     650 credits
  Pro        $99   1,550 credits
  Scale     $299   5,750 credits
```

- Tiers sorted by `price_cents` ascending
- `price_cents` converted to dollar display (`1900` → `$19`)
- Credits formatted with comma separators for readability
- No Stripe purchase links in CLI output (those are dashboard-only per the UX guide)

**Implementation pattern in output.rs:**

The existing `print_response` function takes the format and value. Add a `command: &str` parameter (or pass it via the existing flow) so billing responses can be detected and routed to custom formatters. For JSON format, always passthrough. For human/markdown format, check if command starts with `"billing"` and dispatch accordingly.

### 4. `../mcp-server-cantrip/src/tools.ts` -- Add three billing tools

```typescript
// cantrip_billing_balance
{
  name: "cantrip_billing_balance",
  description: "Check your remaining credit balance. Shows available credits, reserved credits (held by in-progress operations), and total balance.",
  shape: {},
  handler: async () => postCantrip("billing", ["balance"], {}),
},

// cantrip_billing_history
{
  name: "cantrip_billing_history",
  description: "View recent credit transactions. Shows purchases, usage debits, and running balance. Use limit to control how many entries to return.",
  shape: {
    limit: z.number().optional().describe("Maximum entries to return (default: 20)"),
  },
  handler: async (params) => {
    const flags = buildFlags(params);
    return postCantrip("billing", ["history"], flags);
  },
},

// cantrip_billing_tiers
{
  name: "cantrip_billing_tiers",
  description: "View available credit packs and pricing tiers. Shows tier name, price, and credit amount for each pack.",
  shape: {},
  handler: async () => postCantrip("billing", ["tiers"], {}),
},
```

These follow the established pattern: thin wrappers, ~5 lines each, zero business logic.

## System-Wide Impact

- **API surface parity**: After this change, billing is accessible via CLI, MCP server, and dashboard (HTTP). All three use the same `{command, args, flags}` envelope to the same daemon dispatch.
- **Error propagation**: 402 errors from the daemon flow through the existing generic `>= 400` handler in `send_request()`. No special handling needed for v1 -- the daemon returns a clear error message.
- **State lifecycle**: Read-only commands. No state mutation risk.
- **No new dependencies** in either repo.

## Acceptance Criteria

### cantrip-cli

- [x] `cantrip billing` shows credit balance (defaults to balance subcommand) -- `src/cli/mod.rs`, `src/main.rs`
- [x] `cantrip billing balance` shows formatted balance with available/reserved/total -- `src/output.rs`
- [x] `cantrip billing history` shows last 20 transactions formatted as table -- `src/output.rs`
- [x] `cantrip billing history --limit 5` respects the limit flag -- `src/main.rs`
- [x] `cantrip billing tiers` shows pricing tiers sorted by price ascending -- `src/output.rs`
- [x] `--format json` passes through daemon JSON for all three subcommands -- `src/output.rs`
- [x] `--format human` shows billing-specific formatted output -- `src/output.rs`
- [x] `cargo build` succeeds
- [x] `cargo clippy` passes with no warnings

### mcp-server-cantrip

- [x] `cantrip_billing_balance` tool registered and callable -- `src/tools.ts`
- [x] `cantrip_billing_history` tool registered with optional `limit` param -- `src/tools.ts`
- [x] `cantrip_billing_tiers` tool registered and callable -- `src/tools.ts`
- [x] `npm run build` succeeds

## Edge Cases

| Scenario | Expected Behavior |
|---|---|
| Zero balance (new user) | Shows `Available: 0 credits` -- no special treatment |
| Empty history | Shows "No billing history yet." in human format |
| Null description in history entry | Rendered as empty string in human format |
| Reserved credits = 0 | Shows `Reserved: 0 credits` (omit parenthetical) |
| Daemon unreachable | Existing connection error handler applies |
| 401 unauthorized | Existing auth error handler applies |
| `--limit 0` | Passes to daemon, returns empty array |

## Sources & References

### Internal References
- Daemon billing dispatch: `../cantrip/crates/cantrip-server/src/dispatch.rs:843-919`
- Daemon command routing: `../cantrip/crates/cantrip-server/src/dispatch.rs:156`
- Stripe tier config: `../cantrip/stripe.toml`
- StripeConfig types: `../cantrip/crates/cantrip-server/src/stripe_config.rs`
- Credit UX guide: `../cantrip/prd/credit_ux.md`
- Credit pricing strategy: `../cantrip/docs/cantrip_credit_pricing.md`
- Existing CLI patterns: `src/cli/mod.rs` (ApikeyAction), `src/main.rs` (build_request)
- MCP tool pattern: `../mcp-server-cantrip/src/tools.ts`
- Stripe integration plan: `../cantrip/docs/plans/2026-03-07-feat-stripe-credit-pack-purchases-plan.md`
