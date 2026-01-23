# Coding Conventions

**Analysis Date:** 2026-01-23

This document describes conventions observed in the TypeScript reference implementation (`@rlabs-inc/signals`) that should be ported to Rust (`spark-signals`).

## Naming Patterns

**Files:**
- Lowercase with hyphens: `constants.ts`, `tracking.ts`, `equality.ts`, `batching.ts`
- Module grouping by functionality: `core/`, `reactivity/`, `primitives/`, `collections/`
- Each file focused on a single concept or primitive (e.g., `signal.ts` = signal creation, `derived.ts` = derived signals)

**Functions:**
- Lowercase camelCase: `signal()`, `derived()`, `effect()`, `batch()`, `untrack()`
- Internal implementations prefixed with `create`: `createEffect()`, `createDerived()`, `createSignal()`
- Utility functions with action verbs: `updateDerived()`, `markReactions()`, `setSignalStatus()`
- Abbreviations allowed for internal state setters: `setActiveReaction()`, `setNewDeps()`, `setSkippedDeps()`

**Variables (TypeScript):**
- Global state uses single-letter abbreviations when intentional: `f` (flags), `v` (value), `wv` (write version), `rv` (read version)
- Public API uses full names: `value`, `parent`, `first`, `last`, `prev`, `next`
- Setter functions use `set` prefix: `setActiveReaction()`, `setActiveEffect()`, `setUntracking()`
- Increment/decrement functions: `incrementWriteVersion()`, `incrementReadVersion()`, `incrementBatchDepth()`

**Types (TypeScript):**
- PascalCase for interfaces: `Signal`, `Source`, `Reaction`, `Derived`, `Effect`
- Suffixes for type variants: `WritableSignal<T>`, `ReadableSignal<T>`, `DerivedSignal<T>`
- Union types when appropriate: `Value<T> = Source<T> | Derived<T>`
- Generic type parameters: `<T>` for single values, `<T = unknown>` for optional defaults

## Code Style

**Formatting:**
- 2-space indentation
- Line length: ~100 characters (flexible for readability)
- Blank lines between logical sections (2-3 lines for major sections)
- No trailing whitespace

**Comments:**
- Section headers use consistent format with `=====` separators (80 chars wide):
  ```typescript
  // =============================================================================
  // SECTION NAME
  // =============================================================================
  ```
- JSDoc comments above exported functions and types
- Inline comments for complex logic only (avoid obvious comments)
- Example code in JSDoc marked with @example and triple backticks

**JSDoc/TSDoc Style:**
- Function parameters documented: `@param name - description`
- Return types documented: `@returns description`
- Examples provided for public APIs with `@example` blocks
- Type signatures always included
- No need for `@deprecated` unless actively discouraged (prefer deprecation aliases)

Example:
```typescript
/**
 * Read a signal's value and track it as a dependency
 * This is the core of Svelte 5's fine-grained reactivity
 *
 * Key optimizations:
 * 1. Version-based deduplication (rv) prevents duplicate dependencies
 * 2. skippedDeps optimization reuses existing dependency arrays
 *
 * @param signal - The signal to read
 * @returns The current value
 */
export function get<T>(signal: Source<T>): T {
```

## Import Organization

**Order:**
1. Type imports from relative paths
2. Constant imports from relative paths
3. Function imports from relative paths
4. Cross-module function imports (e.g., from `scheduling.ts`)

**Path Aliases:**
- Use relative imports consistently: `import { X } from '../module.js'`
- Imports marked with `.js` extension (ESM)

**Circular Dependency Avoidance:**
- Functions that might create cycles use forward declarations and setters
- Example: `setUpdateDerivedImpl()` allows `tracking.ts` to inject implementation from `derived.ts`
- Lazy-load proxy functions in `signal.ts` to avoid circular deps with `deep/proxy.ts`

## Error Handling

**Patterns:**
- Throw `Error` with descriptive messages for recoverable errors
- Check preconditions at function entry and throw early
- Errors should explain what went wrong and why

**Examples from codebase:**
```typescript
// Cannot write to signals inside a derived
if (activeReaction !== null && (activeReaction.f & DERIVED) !== 0) {
  throw new Error(
    'Cannot write to signals inside a derived. ' +
    'Deriveds should be pure computations with no side effects.'
  )
}

// Uninitialized dependency injection
if (proxyFn === null) {
  throw new Error('state() requires proxy to be initialized. Import from index.ts.')
}
```

**Error context:** Include what was attempted and the constraint violated

## Logging

**Framework:** `console` for debugging (no logging library in signals library)

**Patterns:**
- No logging in production code (side effects not allowed in effects/deriveds)
- Warnings via `console.warn()` for development issues (e.g., uninitialized modules)
- Debug info via `console.log()` is acceptable in utilities, never in reactivity core

## Function Design

**Size:** Functions kept to single logical unit (<100 lines typically, 200 max for complex algorithms)

**Parameters:**
- Keep arity low (2-3 params typical, max 4)
- Use object parameters for configuration/options: `{ equals?: Equals<T> }`
- Type parameters when appropriate: `<T>` for signals, `<T = unknown>` for optional

**Return Values:**
- Synchronous: Return the modified/created value directly
- Void when side effects only: No return needed
- Disposers/cleanups return `DisposeFn` type (function that takes no args, returns void)

**Functional style:**
- Prefer pure functions where possible (esp. in equality, reactivity core)
- Use closures for state capture (e.g., effect cleanup captures previous state)
- Mutable global state allowed in `core/globals.ts` only, with accessor functions
- Avoid mutations except in initialization and cleanup

## Module Design

**Exports:**
- Primitives export both internal (`createX()`) and public API (`X()`)
- Constants defined in `core/constants.ts` are imported everywhere needed
- Type definitions in `core/types.ts` re-exported from `index.ts`

**Barrel Files:**
- `index.ts` exports all public API functions and types
- Internal modules not exported (keep implementation details private)

**Pattern Example:**
```typescript
// In primitives/signal.ts
export function source<T>(...): Source<T> { ... }  // Internal, for library use
export function signal<T>(...): WritableSignal<T> { ... }  // Public API

// In index.ts
export { signal, signal as default } from './primitives/signal.js'
export type { WritableSignal } from './core/types.js'
```

## File Structure

**Typical structure of a primitive module:**

```typescript
// ============================================================================
// @rlabs-inc/signals - [Feature Name]
// [Short description]
// ============================================================================

// Imports (grouped logically)
import type { ... } from '../...'
import { CONSTANTS } from '../...'
import { functions } from '../...'

// =============================================================================
// SECTION 1: INTERNAL IMPLEMENTATION
// =============================================================================

// Forward declarations or setup

// =============================================================================
// SECTION 2: INTERNAL HELPERS
// =============================================================================

// Helper functions

// =============================================================================
// SECTION 3: PUBLIC API
// =============================================================================

/**
 * Main exported function
 */
export function publicAPI(...) {
  ...
}

// Additional exported APIs
publicAPI.variant = function(...) { ... }
```

## Reactivity Core Conventions

**Flags:** Use bitwise operations for compact state representation
```typescript
const flags = signal.f
if ((flags & DIRTY) !== 0) {  // Check if DIRTY flag is set
  // update...
}
signal.f |= CLEAN  // Set CLEAN flag
signal.f &= ~DIRTY  // Clear DIRTY flag
```

**Global state in `core/globals.ts`:**
- All mutable global state declared once at module level
- Setter functions provide access: `setActiveReaction()`, `incrementWriteVersion()`
- Using setters instead of direct assignment enables tracing and validation

**Iterative algorithms:**
- Prefer explicit stack-based iteration over recursion (avoid stack overflow)
- Use `WeakMap` for cycle tracking to prevent memory leaks
- Example: `updateDerivedChain()` uses explicit queue instead of recursion

**Equality checking:**
- Default: `Object.is()` (strict equality)
- Mutable objects: `safeEquals()` (handles NaN, object references)
- Derived/linked: `deepEquals()` (structural equality using Bun.deepEquals)
- Custom: Pass `equals` option to signal/derived creation

## Version-based Deduplication

**Key optimization pattern:**

Each `Source<T>` has:
- `rv` (read version): Prevents tracking same dependency twice in one cycle
- `wv` (write version): Incremented when value changes, used for dirty checking

Each `Reaction` (Effect/Derived) tracks:
- `deps`: Array of dependencies
- When recomputing, increment global `readVersion`, then check `signal.rv < readVersion`

This avoids duplicate dependency entries in a single reaction execution.

---

*Convention analysis: 2026-01-23*
