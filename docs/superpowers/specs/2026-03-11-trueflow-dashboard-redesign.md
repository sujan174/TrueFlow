# TrueFlow Dashboard Redesign (Paper)

## Purpose & Vision
Redesign the TrueFlow dashboard in Paper using extreme restraint on a pitch-black canvas, heavily inspired by early Vercel (zeit.co). The design must feel expensive by what it lacks—no shadows, no gradients, no rounded cards, no colored icons, and no background fills on functional components.

Information hierarchy is driven entirely by typography (size and weight).

## Scope (Phase 1)
1. Login Screen
2. Overview / Home Page (`dashboard/src/app/page.tsx`)

## Architecture & Layout

**Layout Structure:**
- **Sidebar + Top Bar**
- Pure Black Canvas (`#000000`).
- No nested surfaces or card wrappers. Everything sits naked on the canvas.

**Visual Foundation:**
- **Backgrounds:** Pure Black `#000000`.
- **Dividers:** `rgba(255,255,255,0.06)` or `border-white/[0.06]`. Faint lines separating rows/sections.
- **Accents:**
  - Emerald Green (`#34d399`) for positive status indicators (the only use of color for status).
  - Electric Blue (e.g., `#3b82f6` or `#0ea5e9`) for the 2px left border on active sidebar items.
- **Interactions:** Gray/quiet until needed. Hover states use a barely visible `rgba(255,255,255,0.02)` background and reveal gray action icons.

## Typography Rules
1. **No color used to create hierarchy.** Only size, weight, and neutral shades (white/gray) are permitted.
2. **Page Titles:** ~24px, Medium/Semi-bold, White (`#ffffff`).
3. **Body/Data Text:** ~13px or 14px, Regular weight, Zinc-200 (`#e4e4e7`).
4. **Column Headers & Labels:** ~11px, Uppercase, generous tracking/letter-spacing (`tracking-widest`), Zinc-500 (`#71717a`).
5. **IDs, Numbers, Code:** Monospace font (Geist Mono or Paper Mono), ~12px.

## Component Specifics

**1. The Sidebar**
- Fixed width (~220px).
- **NO** section labels with ALL CAPS separators (e.g., no "OBSERVABILITY" text floating between items).
- **Inactive Item:** `text-zinc-500`, no background.
- **Active Item:** White text (`#ffffff`), no background fill, 2px left border in Electric Blue.
- **Hover Item:** Zinc-300 text, `bg-white/[0.03]`.
- Icons must always match the exact color of the text.

**2. The Content Area (Overview Page)**
- **Charts:** Sit directly on the black background. No surrounding borders or background fills.
- **Tables (Deployments/Logs):**
  - Rows sit directly on the page. No card wrapper or border-radius container.
  - Separated by a single 1px faint line.
  - Hover reveals action icons (Zinc-600) and a subtle background change (`bg-white/[0.02]`).

**3. What to Avoid (Anti-patterns)**
- No charts inside bordered card boxes.
- No persistent upgrade banners or commercial pressure in the sidebar.
- No unstyled tooltip cards.
- No gradients anywhere.
- No icons that use colors distinct from their accompanying text.

## New Layouts Established

**High-Density Analytics:**
- Employs a "Whitespace Only" grid (no lines or boxes between charts).
- Top row: 50/50 split (Cost, Tokens). Bottom row: 33/33/33 split.
- Naked sub-nav and filters sitting above the content.

**Data Tables (API Keys & Virtual Keys):**
- Massive naked tables spanning the content area.
- Hierarchy driven entirely by font family (Monospace vs Sans) and color (White for names, Zinc-500 for secondary data).

**Split-View Editor (Policies):**
- 40% left pane for the list of items (faint white background for active state).
- 60% right pane for detail view.
- JSON-Logic displayed as "naked code blocks" (indented Monospace text, no containing boxes).

**Split-View Workspace (Playground):**
- 70% left pane for full-height Chat interaction. Model responses get maximum reading space. User messages have subtle faint borders, no heavy bubble fills.
- 30% right pane for Configuration (Model Selector, System Prompt, Temperature, Max Tokens).
- Collapsible "Debug & Trace" section at the bottom for raw JSON and latency metrics (hidden by default).

## Rule of Thumb
"Is this carrying information or is it decoration?"
- Decoration -> Remove it.
- Information -> Make it quiet but readable.