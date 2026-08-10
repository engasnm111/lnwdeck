# Pet Tooltip Density and Widget Provider Filtering Design

## Goal

Make the desktop pet quota tooltip compact without clipping its glass frame,
make tap-generated speech bubbles fit short text, and keep failed quota
providers out of the floating widget even when they were explicitly selected.

## Tooltip layout

- The quota bubble uses intrinsic content width instead of a fixed 460px width.
- Each quota row places its label, bar, and percentage in max-content columns
  with a 12px gap. No intentional inter-column gap may exceed 20px.
- The bubble remains capped to the pet WebView viewport and keeps its existing
  vertical scrolling behavior, border, radius, shadow, and arrow.
- Long provider names may wrap within a bounded label column; the frame must
  remain completely inside the viewport.

## Speech layout

- Tap speech uses intrinsic width for short text rather than sharing the quota
  bubble's large width.
- Speech is capped at 360px and at the viewport width minus 16px.
- Long speech wraps to at most two lines; short speech does not reserve unused
  horizontal space.

## Widget filtering

- A provider is renderable only when `hasFetchedQuota` accepts it: fresh or
  stale, connected, quota-supported, and not a local estimate.
- Provider selection is applied after this fetched-provider filter. Pinning a
  provider never overrides the data-availability rule.
- The provider picker may continue listing known providers so a temporarily
  failed provider can recover without losing the user's selection.
- If all selected providers are unavailable, the widget shows the localized
  no-quota state instead of a failed provider card.

## Verification

- Add a component regression proving an explicitly selected failed provider is
  hidden while a selected successful provider remains visible.
- Run the focused widget and pet tests, desktop checks, and the release build.
- Inspect the real desktop pet to confirm compact gaps, content-sized speech,
  and an intact frame before publishing.

## Privacy and scope

No credentials, account identifiers, quota values, storage schema, or network
requests change. This work changes presentation and client-side filtering only.
