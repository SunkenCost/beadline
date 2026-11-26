# Frontend Type Safety (`Beadline`)

> Strict TypeScript discipline and Rust↔TypeScript IPC schema parity.

## Compiler Baseline (`tsconfig.json`)

All of the following must remain enabled:
- `"strict": true`
- `"noUncheckedIndexedAccess": true`
- `"noImplicitOverride": true`
- `"noUnusedLocals": true`
- `"noUnusedParameters": true`
- `"exactOptionalPropertyTypes": true` (or explicit `| undefined` unions)

## Rust ↔ TypeScript IPC Contract Parity

1. Every Rust `#[derive(Serialize, Deserialize)]` IPC DTO in `src-tauri/src/models/` uses `#[serde(rename_all = "camelCase")]` and has a 1:1 typed mirror in `src/types/ipc.ts`.
2. Any change to a Rust IPC payload or command signature **must** update `src/types/ipc.ts` and `src/lib/ipc.ts` in the same change set.
3. Discriminated unions are required for bounded enums:
   - `IssueStatus = "open" | "in_progress" | "blocked" | "closed"`
   - `BoardLane = "in_progress" | "ready" | "blocked" | "closed"`
   - `IssueType = "epic" | "task" | "bug" | "feature" | "chore" | "decision" | "spike" | "story" | "milestone"`
   - `ThemePresetId = "warm-claude" | "linear-slate" | "pure-mono"`
   - `AppearanceMode = "light" | "dark" | "system"`
   - `Locale = "zh-CN" | "en"`
4. IPC errors deserialize into a typed `AppErrorDto` (`{ code, message, hint? }`), never opaque strings.

## External Data Validation

- **Theme JSON Import (`BeadlineThemeV1`)**: Validate all imported JSON files with an explicit runtime type guard (`parseThemeJson`) checking `schemaVersion === 1`, valid CSS color strings (hex/rgb/oklch — rejecting `url()`, `expression()`, `<script>`, semicolons), and bounded token keys before applying or persisting.
- **Zero `any`**: Use `unknown` + narrowing guards at IPC/JSON boundaries. Avoid `as unknown as T` casts outside validated parser helpers.
