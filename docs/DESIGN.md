# Design System: lnwdeck — Carbon & Emeralds

## 1. Visual Theme & Atmosphere

A layered-carbon command deck with a single fresh emerald signal. Density is
"Cockpit Dense" (7/10) — data first, every pixel carries information — but the
mood stays calm: deep neutral surfaces, thin structural lines, and one
saturated accent used sparingly for action. Variance sits at "Offset
Asymmetric" (5/10): asymmetric whitespace and split layouts where hierarchy
demands it, never decorative chaos. Motion is "Fluid CSS" (5/10): spring-like
press feedback, calm transitions, no cinematic choreography. The atmosphere is
a well-lit observatory at night — quiet, precise, alive.

## 2. Color Palette & Roles

Dark is the default theme; light is a faithful mirror, never a different brand.

- **Carbon Canvas** (#0B0B0F) — App background, deepest layer. Never pure black.
- **Carbon Panel** (#18181F) — Cards, containers, popovers.
- **Carbon Elevated** (#1F1F28) — Hovered panels, raised surfaces, inputs.
- **Carbon Sidebar** (rgba(17,17,23,0.88)) — Navigation rail, blurred.
- **Zinc Light** (#F4F4F5) — Primary text, headings.
- **Zinc Steel** (#B6B6BF) — Secondary text, descriptions.
- **Zinc Mute** (#82828D) — Metadata, timestamps, placeholders.
- **Whisper Border** (rgba(255,255,255,0.075)) — 1px structural lines.
- **Voice Border** (rgba(255,255,255,0.14)) — Stronger separators, focus edges.
- **Emerald Signal** (#34D399) — THE single accent: primary CTAs, active nav,
  focus rings, success. Saturation ≈ 68% (below the 80% cap).
- **Emerald Bloom** (#4ADE80) — Accent hover state only.
- **Sky Pulse** (#38BDF8) — Info, external links, informational badges.
- **Amber Gauge** (#FBBF24) — Warnings, mid-level quota.
- **Rose Alarm** (#FB7185) — Danger, errors, critical quota.
- **Teal Echo** (#2DD4BF) — Secondary chart series, decorative gradients.

Banned in this palette: purple of any kind, neon gradients, pure black,
warm/cool neutral mixing. Chart series are Emerald → Sky → Amber → Rose →
Teal — no violet slot exists.

## 3. Typography Rules

- **Display:** Segoe UI Variable Display — track-tight, controlled scale
  (1.375rem max in chrome). Hierarchy comes from weight and color, not size.
- **Body:** Segoe UI Variable Text — relaxed leading, 65ch max line length,
  neutral secondary color.
- **Mono:** Cascadia Mono / Consolas — every number in the dense cockpit:
  quota values, token counts, timestamps, cost figures.
- **Banned:** Inter for display work, generic serif fonts (Georgia, Times,
  Garamond), and any font loaded from a CDN at runtime — typefaces are
  platform-native (vendored only).

## 4. Component Stylings

- **Buttons:** Flat, 1px structural border, no outer glow. Primary fills with
  Emerald Signal; secondary is an elevated neutral. Active state pushes down
  1px — tactile press, never a glow. Disabled at 50% opacity.
- **Cards:** 12px radius, carbon panel fill, diffused shadow tinted to the
  canvas. Used only where elevation communicates hierarchy; dense cockpit
  rows separate with border-top dividers instead.
- **Inputs:** Label above, helper below, error text below in Rose Alarm.
  Focus ring in Emerald Signal. No floating labels.
- **Loaders:** Skeletal shimmer matching the layout's real dimensions. No
  generic circular spinners.
- **Empty States:** Composed compositions explaining how to populate the view
  ("No provider has reported data yet. Refresh, or open the dashboard…"),
  never bare "No data" text.
- **Status Chips:** Tinted text on tinted fill — success emerald, warning
  amber, danger rose, muted zinc.
- **Progress Bars:** 6px pill track; unknown-limit windows render a hatched
  track, never a fabricated percentage.

## 5. Layout Principles

- Grid-first responsive architecture; CSS Grid over flexbox math. No
  `calc()` percentage hacks.
- The shell is a fixed navigation rail + content column; content scrolls
  inside its own region (min-height: 0 so flex never clips the scroll end).
- Max-width 1440px centered for content; full-height regions use
  `min-height: 100dvh` semantics.
- Every element occupies its own clear spatial zone — no absolute-positioned
  stacking, no overlap.
- Below 768px every multi-column layout collapses to a single column; the
  sidebar collapses to icons. No horizontal overflow anywhere, ever.
- Touch targets ≥ 44px on interactive elements; body text never below 14px.

## 6. Motion & Interaction

- Spring-like press physics on every interactive element
  (`cubic-bezier(0.2, 0, 0, 1)`, 120–320ms). Never linear.
- Perpetual micro-interaction loops on live cockpit components: the widget
  pet walks and idles, status dots pulse, freshness badges tick.
- Staggered reveals only where a list genuinely benefits; lists never mount
  with a flat pop.
- Animate exclusively `transform` and `opacity` — never `top`, `left`,
  `width`, `height`. All animation is disabled wholesale under
  `prefers-reduced-motion`.

## 7. Anti-Patterns (Banned)

- Purple/blue "AI neon" aesthetics — no purple gradients, no glow buttons.
- Emojis anywhere in shipped UI.
- Inter as a display font; generic serifs (Georgia, Times, Garamond).
- Pure black (#000000) surfaces.
- Oversaturated accents (saturation ≥ 80%).
- Neon/outer glow shadows on interactive elements.
- Gradient text on large headers.
- Custom mouse cursors.
- Overlapping elements — clean spatial separation always.
- 3-column equal card rows; equal symmetric feature grids.
- Generic placeholder names ("John Doe", "Acme", "Nexus").
- Fake round numbers ("99.99%", "50%").
- AI copywriting clichés ("Elevate", "Seamless", "Unleash", "Next-Gen").
- Filler UI text ("Scroll to explore", "Swipe down", bouncing chevrons).
- Fabricated quota percentages — a window without a published limit renders
  a hatched track, never an invented number.
- Centered hero layouts for high-variance sections.
