# TrueFlow Dashboard Redesign (Paper) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recreate the TrueFlow dashboard (Login and Overview pages) natively in Paper using extreme visual restraint and a pitch-black canvas.

**Architecture:** Build visual components directly in Paper using `write_html` and the Paper MCP server. Typography, spacing, and strict color limits (`#000000`, `#ffffff`, `#71717a`, `#e4e4e7`) will drive all hierarchy.

**Tech Stack:** Paper MCP (`write_html`, `create_artboard`, `update_styles`, `set_text_content`), HTML/CSS structure.

---

## Chunk 1: The Login Screen

### Task 1: Create Login Artboard

- [ ] **Step 1: Create the artboard**
  Use `create_artboard` to make a standard Desktop artboard (1440x900px) with a pure black background. Name it "Login".

- [ ] **Step 2: Add the Login Form structure**
  Use `write_html` to add the core login layout centered on the screen.
  - No background card. No border.
  - TrueFlow logo/text at top.
  - Input fields with simple white bottom borders (or very faint full borders `border-white/[0.06]`), no backgrounds.
  - A single white button with black text.

- [ ] **Step 3: Review Checkpoint**
  Use `get_screenshot` to verify the extreme minimalism. Ensure the input fields look like interactive elements despite having no heavy borders or shadows.

---

## Chunk 2: The Overview Page - Foundation

### Task 2: Create Overview Artboard & Top Bar

- [ ] **Step 1: Create the artboard**
  Use `create_artboard` to make a Desktop artboard named "Overview". Set background to `#000000`.

- [ ] **Step 2: Build the Top Bar**
  Use `write_html` to construct a 64px high top bar.
  - Left side: Logo / Project Name (White, Medium weight).
  - Right side: Avatar placeholder or simple user text.
  - Bottom border: `1px solid rgba(255,255,255,0.06)`.

- [ ] **Step 3: Build the Sidebar Foundation**
  Use `write_html` to add a fixed 220px wide sidebar below the top bar on the left.
  - Right border: `1px solid rgba(255,255,255,0.06)`.
  - Add navigation items (Overview, Analytics, API Keys, Policies, etc.).
  - Apply the active state to "Overview": White text, 2px electric blue left border.
  - Apply inactive state to the rest: Zinc-500 text (`#71717a`). NO ALL CAPS section headers.

- [ ] **Step 4: Review Checkpoint**
  Take a screenshot to ensure the spatial relationship between the Top Bar, Sidebar, and the empty content area feels generous and clean.

---

## Chunk 3: The Overview Page - Content

### Task 3: Key Metrics (The Naked Data)

- [ ] **Step 1: Add the Metrics Row**
  In the main content area (beside the sidebar), use `write_html` to add the high-level metrics (e.g., Total Requests, Latency, Error Rate).
  - Do NOT wrap these in boxes or cards.
  - Just place the label (Zinc-500, 11px, Uppercase, Tracking-widest) above the massive data value (White, ~32px, Light/Regular).
  - Space them out generously horizontally.

### Task 4: The Naked Chart

- [ ] **Step 1: Add a placeholder chart**
  Use `write_html` to simulate an area chart (using SVG elements if possible, or a minimal graphic representation).
  - The chart must sit directly on the black background. No container.
  - Axis labels: Zinc-500, 11px.
  - Chart line: White or a very subtle gray.

- [ ] **Step 2: Review Checkpoint**
  Take a screenshot. Confirm the metrics and chart feel grounded without needing structural borders to hold them in place.

---

## Chunk 4: The Overview Page - The Table

### Task 5: The "Table with no Table"

- [ ] **Step 1: Add Table Headers**
  Use `write_html` to lay out column headers (e.g., Time, Model, Status, Latency) below the chart.
  - Font: 11px, Uppercase, `tracking-widest`, Zinc-500.
  - Add a `1px solid rgba(255,255,255,0.06)` border below the headers.

- [ ] **Step 2: Add Table Rows (Audit Logs)**
  Use `write_html` to add 3-4 sample rows of data.
  - Rows sit directly on the background.
  - IDs/Numbers use Monospace font (`font-family: 'Paper Mono', 'Geist Mono', monospace`).
  - Text uses Zinc-200.
  - Status indicator: A single Emerald Green (`#34d399`) dot for "Success".
  - Divider: `1px solid rgba(255,255,255,0.06)` between rows.
  - Right-side actions: Add a quiet 3-dot menu icon in Zinc-600 to simulate hover states.

- [ ] **Step 3: Final Review Checkpoint**
  Take a full screenshot of the Overview page. Verify vertical alignment (lanes), typographic hierarchy, and extreme adherence to the "No Decoration" rule.

---

## Chunk 5: The Analytics Page - High Density Minimalism

### Task 6: Create Analytics Foundation & Top Nav
- [ ] **Step 1: Create the artboard**
  Use `create_artboard` to make a Desktop artboard named "Analytics". Set background to `#000000`.
- [ ] **Step 2: Build Top Bar & Sidebar**
  Use `write_html` to add the standard 64px Top Bar and 220px fixed Sidebar (same structure as Overview).
  - Update top bar breadcrumb to "TrueFlow / acme-corp / Analytics".
  - Update the active sidebar state: Apply the 2px electric blue border and white text to "Analytics". Keep "Overview" as inactive Zinc-500.

### Task 7: Sub-nav & Filters
- [ ] **Step 1: Build Content Header**
  In the main content area, use `write_html` to add a flex row for the sub-navigation ("Overview", "Users", "Errors", "Cache", "Summary").
  - Active item: White text. Inactive items: Zinc-500. No borders or backgrounds.
- [ ] **Step 2: Build Filter Row**
  Below the sub-nav, add a flex row containing a "Search Filter..." input on the left and a "Last 24 hours" dropdown on the right. Both must be naked text with minimal faint borders/icons.

### Task 8: The Naked Grid (Whitespace Only)
- [ ] **Step 1: Top Row Charts (50/50)**
  Use `write_html` to add a flex row with `gap: 64px`. Add two large naked charts ("Cost" and "Tokens Used").
  - Title: 11px, Uppercase, Tracking-widest, Zinc-500.
  - Value: ~32px, Regular, White.
  - Chart grid lines: `rgba(255,255,255,0.06)`. No outer border.
- [ ] **Step 2: Bottom Row Charts (33/33/33)**
  Below the top row, add another flex row with `gap: 64px`. Add three smaller naked charts ("Latency", "Requests", "Unique Users") using the exact same styling logic.

- [ ] **Step 3: Final Review Checkpoint**
  Take a screenshot. Ensure the information density is high (like Portkey) but the visual noise is zero (like Vercel). The spacing (`gap`) should be the only thing separating the sections.

---

## Next.js Implementation Handoff

The design phase in Paper is complete (Screens built: Login, Overview, Analytics, API Keys, Virtual Keys, Policies).

> **Next Step:** To begin translating these Paper designs into the actual `dashboard/src/app` Next.js codebase, invoke a new planning or implementation skill to map out the exact React components and Tailwind classes needed to enforce this design system globally.