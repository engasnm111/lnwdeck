# lnwdeck Product Requirements

## 1. Users

### Primary

- Developer ที่ใช้ AI coding tools หลายตัว
- ผู้ใช้ ChatGPT, Claude, Gemini, Grok หรือ Cursor แบบ Subscription
- ผู้ใช้ API หลาย Provider
- ผู้ใช้ Local model ผ่าน Ollama
- ผู้ดูแลงบประมาณ AI ของตนเองแบบ Local

### Secondary

- Contributor ที่เพิ่ม Provider adapter
- Security reviewer
- Maintainer ที่ดู Release และ Pricing catalog

## 2. Main user journeys

### First run

1. เปิด `lnwdeck`
2. App แสดง Privacy summary
3. ผู้ใช้เลือก Scan local tools
4. App แสดง Provider ที่ตรวจพบและ Data source ที่อ่านได้
5. Provider เริ่มใน Passive/Read-only mode
6. ผู้ใช้เลือกเปิด Browser Helper หรือ Real-time Hook เป็นราย Provider
7. Dashboard แสดงข้อมูลพร้อม Confidence และ Last updated

### Enable real-time hook

1. ผู้ใช้กด `Enable real-time tracking`
2. App แสดงไฟล์ Config, Change preview, สิทธิ์ และผลกระทบ
3. ผู้ใช้ยืนยัน
4. App Backup ไฟล์
5. App เขียนแบบ Atomic
6. App Validate
7. หากไม่ผ่าน App Rollback
8. Audit log เก็บเฉพาะ Metadata ของ Operation

### Browser quota

1. ผู้ใช้ติดตั้ง `lnwdeck Browser Helper`
2. Extension ขอ Host permission เฉพาะ Provider ที่ผู้ใช้เปิด
3. ผู้ใช้เปิดหน้า Usage ที่ Login อยู่
4. Extension extract เฉพาะ normalized usage fields
5. Extension ส่งข้อมูลผ่าน Native Messaging
6. Desktop validate Schema และบันทึก Snapshot
7. Cookie และ Session token ไม่ถูกส่ง

### View analytics

1. เลือก Time range
2. Filter Provider, Tool, Model หรือ Project alias
3. ดู Token, Cost, Requests, Heatmap, Budget และ Forecast
4. Drill down โดยไม่เห็น Prompt/Response/File path
5. Export CSV/JSON แบบ Metadata-only

## 3. Functional requirements

### Dashboard

- Summary cards: Total tokens, Total cost, Requests, Budget status
- Time series: Token และ Cost
- Top providers, tools, models, project aliases
- Current quotas และ Reset countdown
- Alerts และ Data freshness
- Range: hour, day, week, month, custom
- Compare previous period
- Exact/Estimated/Partial/Unknown badge

### Provider management

- Auto-detection
- Capability matrix
- Data source list
- Last success, last error, next retry
- Refresh now
- Enable/disable collector
- Hook preview/install/undo
- Browser permission status
- Credential state โดยไม่แสดง Secret

### Analytics

- Raw event ingestion
- Hourly and daily rollups
- Heatmap
- Model and provider breakdown
- Cache token categories
- Budget tracking
- Forecast
- Export
- Retention settings
- Recalculate cost on explicit user action

### Notifications

- Quota threshold
- Budget threshold
- Quota reset detected
- Provider auth expired
- Data stale
- Adapter disabled after repeated crash
- Update ready to restart

### System tray

- Current monthly token and cost
- Budget percentage
- Provider with highest usage
- Last updated
- Refresh all
- Open Dashboard
- Open/close Floating Widget
- Pause collection
- Exit

### Floating widget

- Always-on-top
- Drag, resize, opacity
- Compact rows for Provider/Quota/Cost
- Remember monitor and position
- Snap to screen edges
- Lock position
- Hide from Alt+Tab where Windows API permits
- No click-through in v0.1

### Settings

- Startup
- Theme
- Currency/display
- Refresh policy
- Retention
- Notifications
- Pricing overrides
- Browser Helper
- Adapter permissions
- Backup/export/import
- Update behavior
- Privacy and diagnostics

## 4. Quality attributes

### Reliability

- Collector isolation
- Last-good cache
- Idempotent ingestion
- Transactional migrations
- Retry with jittered exponential backoff
- No global crash from malformed provider data

### Performance targets

Targets วัดบน Reference x64 machine และใช้เป็น Regression budget:

- Tray idle CPU median below 1%
- Background collector memory target below 80 MB
- Whole app idle memory target below 220 MB
- Main Dashboard warm open below 2.5 seconds
- Tray popup visible below 400 ms after click
- Incremental sync of unchanged logs performs no row insert
- Database query for 30-day overview below 250 ms at 1 million events
- x86 whole app target below 180 MB and disables unsupported adapter runtime explicitly

### Accessibility

- Keyboard navigation
- Semantic labels
- Contrast WCAG AA
- Reduced motion
- Screen reader names for charts and summary tables
- Do not rely on color alone

## 5. v0.1 acceptance criteria

- Provider detection returns deterministic results from fixtures
- At least one Provider works for each collection mode:
  - Local log
  - Hook
  - Official API
  - Browser Helper
  - Local provider API
- All 10 Provider groups have a documented capability matrix
- Data schema rejects Prompt/Response/Path fields
- Dashboard, Tray และ Floating Widget use the same read model
- Export contains no forbidden fields
- Update workflow supports download, verify, wait, restart
- Portable mode never writes application data outside portable data directory except OS-required WebView/cache behavior documented to the user
