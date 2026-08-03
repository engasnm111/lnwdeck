# lnwdeck Project Charter

## Vision

สร้าง Universal AI Usage Tracker แบบ Open Source สำหรับ Windows ที่ช่วยให้ผู้ใช้เห็น Token, Cost, Quota, Reset time และแนวโน้มการใช้งาน AI ทั้งหมดจากจุดเดียว โดยข้อมูลสำคัญไม่ออกจากเครื่อง

## Problem statement

ผู้ใช้ AI หลายเครื่องมือจำเป็นต้องเปิด Dashboard หลายแห่งและอ่าน Log หลายรูปแบบ บางบริการมี API บางบริการมีเฉพาะหน้าเว็บ บาง CLI เก็บ JSONL หรือ SQLite ภายในเครื่อง การรวมข้อมูลด้วยวิธีเดียวจึงไม่เพียงพอ

`lnwdeck` แก้ปัญหาด้วย Hybrid collection:

- Local session logs
- Hooks และ File watchers
- Official APIs
- Browser Helper
- Local provider APIs
- Sandboxed community adapters

## Product principles

1. Local-first, local-only ใน v0.1
2. Metadata-only
3. Read-only เป็นค่าเริ่มต้น
4. Consent ก่อนแก้ Config หรือเปิดสิทธิ์
5. Adapter isolation
6. Offline-capable Dashboard
7. Exactness มีป้ายกำกับ: Exact, Estimated, Partial, Unknown
8. Core ต้องไม่พังเมื่อ Provider ใด Provider หนึ่งเปลี่ยน
9. Windows-first แต่ไม่ผูก Domain model กับ Windows
10. Open source และตรวจสอบได้

## v0.1 goals

- แสดง Current usage และ Quota ใน Dashboard, Tray และ Floating Widget
- เก็บ History และสร้าง Analytics เต็มรูปแบบ
- รองรับ Provider หลัก 10 กลุ่ม
- ตรวจพบเครื่องมือในเครื่องอัตโนมัติ
- Passive collection ก่อนติดตั้ง Hook
- Hook install แบบ Preview/Backup/Validate/Rollback
- Budget, Alert, Forecast และ Export
- Hybrid pricing แบบ Offline + Update + Override
- Edge/Chrome Browser Helper
- Installer, Portable และ Auto-update
- เปิด SDK และ Sandbox สำหรับ Community Adapter

## Provider groups สำหรับ v0.1

1. Claude Code / Claude Web
2. Codex CLI / ChatGPT
3. Cursor
4. Gemini CLI / Gemini Web
5. GitHub Copilot
6. OpenCode
7. Grok Build / Grok Web
8. Kiro
9. Ollama
10. OpenRouter

Provider อาจเปิดใช้ Capability ไม่เท่ากัน ตัวอย่างเช่น Ollama ไม่มี Subscription quota และ Web provider บางตัวอาจไม่มี Token breakdown

## Non-goals ของ v0.1

- Cloud sync
- Account ของ lnwdeck
- Mobile app
- macOS/Linux native release
- เก็บ Prompt/Response
- Remote control AI tools
- ซื้อหรือเพิ่ม Quota ให้ผู้ใช้
- Intercept HTTPS traffic
- Credential harvesting
- Marketplace ที่รัน Native DLL จากบุคคลภายนอก
- Forecast แบบ Machine learning

## Success criteria

- ผู้ใช้ติดตั้งและเห็น Provider ที่ตรวจพบภายใน 60 วินาที
- App ใช้งานได้โดยไม่สร้างบัญชี
- Dashboard เปิดได้เมื่อ Offline
- Provider ที่ Error ไม่ทำให้ App ล่ม
- ข้อมูลต้องระบุ Source, Freshness และ Confidence
- Privacy contract tests ต้องยืนยันว่าไม่มี Sensitive content ถูก Persist
- Tier 1 builds ผ่าน CI สำหรับ x64 และ ARM64
- x86 เปิด Dashboard, SQLite, built-in core และ Adapter ที่ประกาศรองรับได้
- Auto-update ตรวจ Signature ก่อนเตรียม Restart
- Release ไม่มี Critical/High security finding ที่ยังเปิดอยู่

## Product naming

- Brand: `lnwdeck`
- Executable: `lnwdeck.exe`
- CLI: `lnwdeck`
- App identifier: `app.lnwdeck.desktop`
- Native messaging host: `app.lnwdeck.browser_helper`
- Data directory installed mode: `%LOCALAPPDATA%\lnwdeck`
- Portable data directory: `<app-directory>\lnwdeck-data`
