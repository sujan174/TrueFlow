# TrueFlow Dashboard Next.js Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Translate the approved minimalist "Early Vercel" Paper designs into the actual Next.js dashboard codebase.

**Architecture:**
The dashboard uses Next.js App Router, Tailwind CSS, and Shadcn UI. We will aggressively strip out existing "heavy" Shadcn styles (cards, borders, shadows) and replace them with our new `border-white/[0.06]`, pure black (`#000000`), and typography-driven hierarchy (`text-zinc-500`, `text-e4e4e7`).

**Tech Stack:** Next.js (App Router), Tailwind CSS, Lucide Icons (stripping colors), Recharts.

---

## Chunk 1: Global Foundation & Layout

### Task 1: Update Global Theme and Layout Shell

**Files:**
- Modify: `dashboard/src/app/globals.css`
- Modify: `dashboard/src/app/layout.tsx`
- Modify: `dashboard/tailwind.config.ts` (if exists, or global CSS variables)

- [ ] **Step 1: Enforce pure black background globally**
  In `globals.css`, ensure `body` and `:root` backgrounds are set to `#000000`. Remove any `bg-background` variables that map to lighter grays.
- [ ] **Step 2: Establish the Top Bar & Sidebar shell**
  In `layout.tsx` (or the main navigation component if abstracted), implement the new layout structure:
  - 64px Top bar with `border-b border-white/[0.06]`.
  - 220px fixed left sidebar with `border-r border-white/[0.06]`.
  - Remove all grouping/categorization text (e.g., "OBSERVABILITY") from the sidebar.
  - Update sidebar active state: `border-l-2 border-blue-500 text-white bg-transparent`.
  - Update sidebar inactive state: `text-zinc-500 hover:text-zinc-300 hover:bg-white/[0.03]`.

### Task 2: Implement "Naked Table" Component

**Files:**
- Create/Modify: `dashboard/src/components/ui/table.tsx` (or create a custom `NakedTable` wrapper).

- [ ] **Step 1: Strip Shadcn Card wrappers**
  Modify the default table components so they do not render inside a `Card` or have outer borders.
- [ ] **Step 2: Apply Vercel styling**
  - Table Header (`th`): `text-[11px] uppercase tracking-widest text-zinc-500 font-normal border-b border-white/[0.06] pb-3`.
  - Table Row (`tr`): `border-b border-white/[0.06] hover:bg-white/[0.02] transition-colors`.
  - Table Cell (`td`): `py-4 text-[13px] text-zinc-200`.

---

## Chunk 2: Core Pages Refactor

### Task 3: Redesign Login Page

**Files:**
- Modify: `dashboard/src/app/login/page.tsx`

- [ ] **Step 1: Strip existing styles**
  Remove any Card, CardHeader, CardContent wrappers.
- [ ] **Step 2: Build naked form**
  - Title: `text-2xl font-medium tracking-tight text-white`.
  - Labels: `text-[11px] uppercase tracking-widest text-zinc-500`.
  - Inputs: `bg-transparent border-b border-white/[0.06] border-x-0 border-t-0 rounded-none focus:ring-0`.
  - Submit Button: `bg-white text-black font-medium rounded-none hover:bg-zinc-200`.

### Task 4: Redesign Overview Page (Home)

**Files:**
- Modify: `dashboard/src/app/page.tsx`

- [ ] **Step 1: Update Metrics Row**
  Remove metric cards. Render metrics as:
  `flex flex-col gap-2`
  Label: `text-[11px] uppercase tracking-widest text-zinc-500`
  Value: `text-3xl text-white font-normal`
- [ ] **Step 2: Update Chart**
  Remove chart container borders/backgrounds. Update Recharts config to use `stroke="#ffffff"` for lines, remove cartesian grid backgrounds, and use `rgba(255,255,255,0.06)` for grid lines.
- [ ] **Step 3: Update Recent Logs Table**
  Implement the `NakedTable` from Task 2. Use a simple `<div className="w-1.5 h-1.5 rounded-full bg-emerald-400" />` for success status.

---

## Chunk 3: High-Density Pages

### Task 5: Redesign Analytics Page

**Files:**
- Modify: `dashboard/src/app/analytics/page.tsx`

- [ ] **Step 1: Implement Sub-nav & Filters**
  Add the naked text sub-nav (`Overview`, `Users`, etc.) above the content. Add a borderless search input (`border-b border-white/[0.06]`) and dropdown.
- [ ] **Step 2: Implement "Whitespace Grid"**
  Layout the charts using flexbox or CSS grid with large gaps (`gap-16` or similar) and absolutely no borders or backgrounds between them.
  - Top row: 2 charts (`grid-cols-2`).
  - Bottom row: 3 charts (`grid-cols-3`).

### Task 6: Redesign API Keys & Virtual Keys Pages

**Files:**
- Modify: `dashboard/src/app/api-keys/page.tsx`
- Modify: `dashboard/src/app/virtual-keys/page.tsx`

- [ ] **Step 1: Update Headers**
  Clean up the page headers. Add the unstyled "Create" button aligned to the right.
- [ ] **Step 2: Implement Naked Tables**
  Convert the existing data grids to use the new `NakedTable` styling. Ensure ID/Key columns use monospace fonts (`font-mono text-[12px]`).
- [ ] **Step 3: Clean up Badges**
  Remove Shadcn `Badge` components (colored pills) for roles/scopes. Render them as simple stacked text: `text-[13px] text-zinc-200` for the primary role, `text-[12px] text-zinc-500` for the scopes.

---

## Chunk 4: Complex Layouts

### Task 7: Redesign Policies Page (Split View)

**Files:**
- Modify: `dashboard/src/app/policies/page.tsx`

- [ ] **Step 1: Build Split Layout**
  Change the page layout to a full-height flex container.
  - Left pane: `w-[380px] border-r border-white/[0.06] shrink-0`.
  - Right pane: `flex-1 overflow-y-auto px-16 py-12`.
- [ ] **Step 2: Build Policy List (Left)**
  Map over policies. Render as stacked divs with no borders.
  Active state: `bg-white/[0.04]`. Inactive hover: `hover:bg-white/[0.02]`.
- [ ] **Step 3: Build JSON-Logic Detail (Right)**
  Display the JSON logic in a `font-mono text-[13px] leading-relaxed text-zinc-300` container without a background box. Use nested divs or pre/code blocks with careful indentation to mimic the Paper design.

### Task 8: Redesign Playground Page (Split View Workspace)

**Files:**
- Modify: `dashboard/src/app/playground/page.tsx`

- [ ] **Step 1: Build Split Layout**
  Change the page layout to a full-height flex container.
  - Left pane (Chat): `flex-[7] relative overflow-y-auto`.
  - Right pane (Config): `flex-[3] border-l border-white/[0.06] flex flex-col`.
- [ ] **Step 2: Build Chat Interface (Left)**
  - Message container: `flex flex-col gap-10 px-12 py-8`.
  - User messages: `self-end max-w-[80%] border border-white/[0.06] bg-white/[0.03] p-4 rounded`. No heavy bubble colors. Add simple prefix `User` in `text-[11px] uppercase tracking-widest text-zinc-500`.
  - Assistant messages: `self-start max-w-[80%] p-4`. Text `text-zinc-200`. Add prefix `Claude 3.5 Sonnet` (or model name).
  - Input box: Positioned absolute/fixed at bottom. `border border-white/[0.12] bg-black`.
- [ ] **Step 3: Build Configuration Pane (Right)**
  - Parameters: Naked dropdown for Model, simple textarea for System Instructions.
  - Sliders: Use minimal custom styling (a thin 1px line `bg-white/[0.06]` with a small white thumb).
  - Debug & Trace: Collapsible section at bottom, hidden by default. Use simple toggle: `text-[11px] uppercase tracking-widest text-zinc-500`.