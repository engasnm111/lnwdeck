# lnwdeck UI/UX Specification

## 1. Brand

- Product name: `lnwdeck`
- Working tagline: `Universal AI Usage Tracker`
- Default theme: Dark
- Optional theme: Light and Follow system
- Visual direction: modern, compact, data-first, suitable for long-running desktop utility
- No decorative animation that consumes continuous CPU

## 2. Main navigation

1. Overview
2. Providers
3. Analytics
4. Costs
5. Budgets
6. Models
7. Alerts
8. Adapters
9. Settings
10. System

## 3. Overview

Top bar:

- Page title
- Last updated
- Data freshness
- Refresh all
- Update ready indicator
- Window controls

Summary cards:

- Total tokens
- Total cost
- Requests
- Budget status

Charts:

- Token usage
- Cost actual vs estimated
- Provider share
- Quota windows

Tables:

- Top providers
- Top models
- Recent alerts
- Data quality summary

## 4. Provider page

Provider card shows:

- Name and icon
- Detected source
- Enabled status
- Capabilities
- Current quota
- Reset time
- Last success
- Health
- Confidence
- Refresh now
- Configure
- Enable real-time tracking
- Disable
- View source and permissions

Hook preview must show:

- Target file
- Existing hash
- Change diff
- Backup location description
- Rollback behavior
- Confirm/Cancel

Do not show raw credential or raw file content beyond the minimal relevant diff

## 5. Analytics

Controls:

- Date range
- Compare previous period
- Provider/tool/model/project alias filters
- Token categories
- Confidence filter
- Export

Views:

- Time series
- Heatmap
- Breakdown
- Quota history
- Cost coverage
- Data quality

Every chart has:

- Accessible summary
- Table alternative
- Empty state
- Partial-data warning
- Timezone label

## 6. Tray popup

Width target: 320–380 px

Content:

- `lnwdeck`
- This month tokens
- This month cost
- Budget progress
- Requests
- Highest-usage provider
- Quotas near limit
- Last updated
- Open Dashboard
- Refresh now

Tray menu:

- Open Dashboard
- Toggle Floating Widget
- Pause/Resume collection
- Check for updates
- Start with Windows
- Exit

Closing Dashboard does not exit application

## 7. Floating widget

Modes:

- Provider summary
- Quota list
- Cost summary
- Compact one-row mode

Controls:

- Drag
- Resize
- Opacity
- Always-on-top
- Lock
- Refresh
- Open Dashboard
- Close widget

Persistence:

- Position per monitor
- Size
- Mode
- Opacity
- Lock state

Out-of-bounds recovery moves widget into visible work area

## 8. Responsive layout

- Wide: fixed sidebar + multi-column cards
- Medium: collapsible sidebar
- Small desktop window: stacked cards and horizontal table scroll
- Compact: essential cards only
- Tray and widget are dedicated layouts, not scaled Dashboard pages

## 9. UI states

Every data component supports:

- Initial loading
- Refreshing with last-good data
- Empty
- Not configured
- Permission required
- Stale
- Partial
- Error
- Unsupported
- Offline

## 10. Notifications

Notifications are actionable and rate-limited.

Examples:

- `Claude weekly quota reached 90%`
- `OpenRouter monthly budget reached 80%`
- `Codex data has not refreshed for 30 minutes`
- `lnwdeck update is ready. Restart when convenient.`

Repeated notifications for same condition are suppressed until state changes

## 11. Accessibility and localization

- Thai and English-ready string system
- v0.1 may ship English UI first, but no hard-coded strings in components
- Keyboard access
- Screen reader labels
- High contrast
- Reduced motion
- Locale-aware date, number and currency
- Store timestamps in UTC, display in user timezone
