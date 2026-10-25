# AGENTS.md — web

The console. Vite, React, Tailwind v4, Feature-Sliced. Built into the binary by `cargo build`.

## Commands

```sh
npm run check     # tsc, then the layering check. Both must pass
npm run build
npm run dev       # proxies /api to 127.0.0.1:8787
```

`check-layers.mjs` walks every import and fails on a reach upward, or a sideways reach into
another slice that does not go through a public API. It runs inside `build`, which is what
cargo runs for a release, so the arrangement cannot quietly rot.

## Layers

`app → pages → widgets → features → entities → shared`. Import downward only.

- Every slice has an `index.ts`, and nothing imports past one.
- Two entities that genuinely share a type do it through `@x`, so the sharing is visible.
- A feature never imports another feature. If two need composing, that composition is a widget
  — `widgets/register-account` composes the API-key form and the sign-in flow.

## Screens

- A list of things is a `Split`: the list on the left, an `Inspector` on the right. What is
  selected lives in the URL (`/accounts/:id?`), so it survives a reload and can be linked to.
  `useSelection` reads and moves it, `useRowKeys` gives the list J, K and Esc.
- Actions on one thing go in its inspector and its context menu, not as buttons on every row.
- No boxes around sections inside a page. A `Section` is a heading and its content; hairlines
  separate them. Boxes are for dialogs and settings groups.
- A column that hides when the inspector narrows the list uses a container query
  (`hidden @3xl:table-cell`). Never hide a column in a table with group rows: the group row's
  `colSpan` then counts a column that is not there.

## State

zustand, never `localStorage` directly. The `persist` middleware handles storage.

- `shared/model/session` — whether someone has to sign in.
- `shared/model/series` — which colour slot an entity holds, per chart.
- `shared/model/commands` — whether the command menu is open, and the commands pages have
  registered with `useCommand` while they are mounted.
- `shared/ui/confirm-dialog` — the question `confirmed()` is waiting on. Never `window.confirm`.
- `widgets/app-shell/model/sheet` — whether the keyboard shortcut sheet is open.
- `features/toggle-theme/model` — dark or light.
- `entities/request/model/live` — requests in flight.

Nothing app-wide is passed down as a prop.

## TypeScript

No type assertions. No `as`, no `!`. Narrow by checking the field; use `as const` tuples so an
index is known rather than possibly undefined. The single place untyped data enters is
`shared/api/client.ts`, and it enters typed.

## Charts

Drawn by hand with `d3-scale`, not a charting library, because each of these is a default worth
fighting elsewhere:

- The table is the legend. No legend boxes.
- No value labels on bars.
- One axis.
- The right edge is now.
- Colour follows the entity, never its rank. Adding a provider must not recolour the others.

Tiles are rates, ratios, and counts that need a decision — never the integral of the chart
below them. Totals live once, in the table footer, where they cannot drift from it.

## Saying what is true

- "Not reported" and "not observed yet" are different states and must read differently.
- Latency says it is measured over successful requests.
- Quota says when it was last seen, because it refreshes from real traffic and being stale is
  structural rather than a bug.
- An empty screen says which emptiness it is: no accounts, no requests, or no matches.
