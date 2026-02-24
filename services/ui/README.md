# Ferrum Web UI

React 18 + TypeScript + Vite frontend for the Ferrum GA4GH platform.

## Stack

- **Vite** – build tool
- **React 18** + **TypeScript**
- **TailwindCSS** + **shadcn/ui** (Radix-based components)
- **TanStack Query** – server state
- **TanStack Router** – routing
- **Recharts** – charts
- **Zustand** – client state (theme, auth)
- **Monaco Editor** – JSON workflow params (optional)

## Pages

| Path | Purpose |
|------|--------|
| `/` | Dashboard – summary cards, storage donut, run history, health badges |
| `/data` | Data Browser – DRS objects table, filters, upload, object detail |
| `/workflows` | Workflow Center – WES runs list, submit form, run detail + logs |
| `/tools` | Tool Registry – TRS tools grid |
| `/beacon` | Beacon Explorer – variant query, chromosome range, results |
| `/access` | Access Management – datasets, visa grants, passport viewer |
| `/settings` | Settings – config viewer, storage, keys, profile |

## Development

```bash
npm install
npm run dev
```

Runs at `http://localhost:5173`. API requests are proxied to `http://localhost:8080` (gateway).

## Build

```bash
npm run build
```

Output in `dist/`. Serve with the gateway under `/ui` or any static host.

## Auth

GA4GH Passport JWT is kept in memory only (Zustand store); set via `useAuthStore.getState().setPassport(jwt)`. The API client sends `Authorization: Bearer <jwt>` when present.
