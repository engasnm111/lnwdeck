# Design System: lnwdeck — Premium Dark Futuristic Glassmorphism

## 1. Visual Theme & Atmosphere

A layered glass command deck floating over a near-black navy atmosphere.
Density is "Cockpit Dense" (7/10) — data first, every pixel carries
information — but the mood is premium and calm: deep navy surfaces, frosted
glass panels, thin glowing borders, and a cyan → blue → violet accent family
used with restraint. Variance sits at "Offset Asymmetric" (5/10):
asymmetric whitespace and split layouts where hierarchy demands it, never
decorative chaos. Motion is "Fluid CSS" (5/10): gentle lifts, soft glows,
sweep micro-interactions, no cinematic choreography. The atmosphere is a
private observation deck at night — elegant, immersive, technology-driven.

## 2. Color Palette & Roles

Dark is the default theme; light is a faithful mirror, never a different brand.

- **Abyss Ink** (#05070F) — App background, deepest layer. Never pure black.
- **Glass Surface** (white 3–6% + backdrop blur) — Cards, panels, popovers,
  the navigation rail and the topbar. Soft background blur, thin
  semi-transparent borders, layered elevation.
- **Primary Text** (#E8EDF7) — Headings, key values.
- **Secondary Text** (#8D9BB5) — Descriptions, body copy.
- **Muted Text** (#64708C) — Metadata, timestamps, placeholders.
- **Neon Cyan** (#22D3EE) — THE primary accent: active nav, focus rings,
  primary info, chart series start.
- **Electric Blue** (#2563EB) — Secondary accent: secondary charts,
  informational highlights, gradient depth.
- **Violet** (#8B5CF6) — Supporting accent: chart series end, decorative
  gradients, hover glow ends.
- **Warm CTA** (Amber #F59E0B → Red #EF4444) — Only for primary
  call-to-action buttons where user attention is required. Never used for
  status, text, or decoration.
- **Semantic status**: success emerald #34D399, warning amber #FBBF24,
  danger rose #F87171, info sky #38BDF8 — each with a tinted translucent fill.

Glow vocabulary: cyan → violet. Hover states may introduce a subtle
cyan-to-violet glow; interactive emphasis is built from layered translucent
gradients, never harsh outer glows.

## 3. Typography Rules

- **Display:** Segoe UI Variable Display — track-tight, controlled scale.
  Hierarchy comes from weight, size and the shimmer effect on page titles.
- **Body:** Segoe UI Variable Text — relaxed leading, 65ch max line length,
  neutral secondary color.
- **Mono:** Cascadia Mono / Consolas — every number in the dense cockpit:
  quota values, token counts, timestamps, cost figures.
- **Banned:** generic serif fonts (Georgia, Times, Garamond), and any font
  loaded from a CDN at runtime — typefaces are platform-native (vendored only).
- Important headlines (page titles) may carry a subtle animated gradient
  shimmer sweeping cyan → violet → white, disabled under
  `prefers-reduced-motion`.

## 4. Component Stylings

- **Glass panels:** frosted surface — white 3–6% translucent fill, 18–28px
  backdrop blur, 1px semi-transparent borders with a brighter top edge,
  layered shadows tinted to the canvas. Hover may raise the panel 2px and
  shift the border toward a faint cyan/violet glow.
- **Buttons:** Glass secondary; the primary variant is the Amber → Red warm
  gradient with a shine sweep on hover and a soft lift. Active state pushes
  down 1px. Disabled at 50% opacity. Danger is a rose outline on glass.
- **Cards:** 12–16px radius, glass fill, diffused shadow tinted to the
  canvas; dense cockpit rows separate with border-top dividers instead.
- **Inputs:** Glass fill, label above, helper below, error text below in
  Rose. Focus ring in Neon Cyan. No floating labels.
- **Loaders:** Skeletal shimmer matching the layout's real dimensions. No
  generic circular spinners.
- **Empty States:** Composed compositions explaining how to populate the view,
  never bare "No data" text.
- **Status Chips:** Tinted text on tinted translucent fill — success emerald,
  warning amber, danger rose, muted zinc.
- **Progress Bars:** 6px pill track with a subtle cyan gradient fill;
  unknown-limit windows render a hatched track, never a fabricated percentage.

## 5. Background

The canvas is a fixed atmosphere layer behind all content:

- Near-black navy base (#05070F) with soft radial gradients.
- Large blurred gradient orbs in cyan, blue and violet, drifting gently
  (transform/opacity only, disabled under reduced motion).
- A very subtle grid pattern, faded at the edges with a radial mask.
- A low-opacity SVG noise texture (inline data URI, no network requests).

## 6. Layout Principles

- Grid-first responsive architecture; CSS Grid over flexbox math.
- The shell is a fixed glass navigation rail + content column; content
  scrolls inside its own region (min-height: 0 so flex never clips the end).
- Max-width 1440px centered for content; full-height regions use
  `min-height: 100dvh` semantics.
- Every element occupies its own clear spatial zone — no absolute-positioned
  stacking over content.
- Below 768px every multi-column layout collapses to a single column; the
  sidebar collapses to icons. No horizontal overflow anywhere, ever.
- Touch targets ≥ 44px on interactive elements; body text never below 14px.

## 7. Motion & Interaction

- Spring-like press physics on every interactive element
  (`cubic-bezier(0.2, 0, 0, 1)`, 120–320ms). Never linear.
- Micro-interaction loops on live cockpit components: status dots pulse,
  freshness badges tick, CTA buttons carry a shine sweep.
- Staggered reveals only where a list genuinely benefits; lists never mount
  with a flat pop.
- Animate exclusively `transform` and `opacity` — never `top`, `left`,
  `width`, `height`. All animation is disabled wholesale under
  `prefers-reduced-motion`.

## 8. Anti-Patterns (Banned)

- Emojis anywhere in shipped UI.
- Generic serifs (Georgia, Times, Garamond).
- Pure black (#000000) surfaces; everything sits on Abyss Ink (#05070F).
- Oversaturated neon canvas (the accent family stays the cyan/blue/violet
  triad; the warm gradient is reserved for CTAs only).
- Harsh outer glows on text; glows belong to borders and panels.
- Gradient text on large headers — page titles shimmer once, they never
  rainbow.
- Custom mouse cursors.
- Overlapping elements — clean spatial separation always.
- Generic placeholder names ("John Doe", "Acme", "Nexus").
- Fake round numbers ("99.99%", "50%").
- AI copywriting clichés ("Elevate", "Seamless", "Unleash", "Next-Gen").
- Filler UI text ("Scroll to explore", bouncing chevrons).
- Fabricated quota percentages — a window without a published limit renders
  a hatched track, never an invented number.
- Centered hero layouts for high-variance sections.
