# AGENTS.md — inwdeck Engineering Rules

เอกสารนี้เป็นกฎบังคับสำหรับ AI coding agent และ Contributor ทุกคน

## 1. วิธีทำงาน

1. อ่านเอกสารใน `docs/` ก่อนแก้โค้ด
2. ทำงานครั้งละหนึ่ง Task จาก Implementation plan
3. ห้ามสร้าง Subagent เว้นแต่ผู้ใช้สั่งโดยตรง ให้ทำงาน Inline เป็นค่าเริ่มต้น
4. ก่อนแก้ไฟล์ ให้สรุปขอบเขต Task และรายชื่อไฟล์ที่จะสร้างหรือแก้
5. ใช้ Test-Driven Development:
   - เขียน Test ที่ล้มเหลวก่อน
   - รันเพื่อยืนยันว่าล้มเหลวด้วยเหตุผลที่ถูกต้อง
   - เขียน Implementation ขั้นต่ำ
   - รัน Test ที่เกี่ยวข้อง
   - รัน Quality gates ของ Workspace
6. หยุดและรายงานเมื่อ Task ผ่าน Acceptance criteria ห้ามไหลไปทำ Task ถัดไปเอง
7. Commit ต้องเล็ก อ่านง่าย และมีจุดประสงค์เดียว
8. ห้ามใช้ข้อความแทน Implementation จริง ห้ามเว้นฟังก์ชันว่าง และห้ามปิด Test เพื่อให้ CI ผ่าน

## 2. ขอบเขต Product ที่ห้ามเปลี่ยนเอง

- ชื่อ Product คือ `inwdeck`
- Local-only เป็นค่าเริ่มต้นและ v0.1 ไม่มี Cloud account หรือ Cloud sync
- เก็บเฉพาะ Metadata
- ห้ามเก็บ Prompt, Response, Source code, File content, File name หรือ Absolute path
- Project identity ต้องใช้ Alias และ Identifier แบบ keyed hash
- Hooks ต้องเริ่มจาก Passive mode และต้องขออนุมัติผู้ใช้ก่อนติดตั้ง
- ทุกการแก้ Config ต้อง Preview, Backup, Validate และ Rollback ได้
- Browser Helper ห้ามส่ง Cookie หรือ Session token ออกจาก Browser
- Community Adapter ต้องอยู่ใน Sandbox และไม่มีสิทธิ์โดยปริยาย
- x64 และ ARM64 เป็น Tier 1; x86 เป็น Compatibility Tier
- Windows 10 ขั้นต่ำคือ 22H2

## 3. Architecture boundaries

- `crates/domain`: Domain types และ invariants เท่านั้น
- `crates/application`: Use cases และ orchestration
- `crates/storage`: SQLite, migrations และ repositories
- `crates/security`: Credential, hashing, redaction และ permission checks
- `crates/provider-runtime`: Adapter lifecycle, scheduling และ isolation
- `crates/providers/*`: Built-in provider adapters
- `apps/desktop/src`: React UI
- `apps/desktop/src-tauri`: Tauri commands และ native integration
- `apps/browser-extension`: Chromium Manifest V3 extension
- `schemas`: JSON Schema และ WIT contracts ที่เป็น source of truth
- UI ห้าม Query SQLite โดยตรง ต้องผ่าน Tauri command/use case
- Provider adapter ห้ามเขียน Database โดยตรง ต้องคืน Normalized batch ให้ Core
- Secret ห้ามไหลเข้า UI, Log, Analytics event หรือ Adapter ที่ไม่ได้รับสิทธิ์

## 4. Security rules

- Deny-by-default สำหรับ File, Network, Credential และ Hook permissions
- ใช้ Windows Credential Manager หรือ Tauri Stronghold ตาม Threat model
- ห้ามเก็บ Secret ใน `.env`, JSON, SQLite หรือ Source code
- Log ต้องผ่าน Redaction ก่อนเขียน
- Native Messaging ต้องตรวจ Extension origin แบบ allowlist
- Network destination ต้องตรงกับ Domain ที่ประกาศใน Adapter manifest
- ใช้ HTTPS เท่านั้น ยกเว้น Loopback/Local provider ที่ผู้ใช้เปิดเอง
- Database migration ต้อง Backup ก่อนและ Transactional
- Update artifact ต้องตรวจ Signature ก่อนติดตั้ง
- Dependency ใหม่ต้องอธิบายเหตุผลและตรวจ License

## 5. Coding standards

### Rust

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- หลีกเลี่ยง `unwrap()` และ `expect()` ใน Production path
- Error ต้องมี typed error และ context ที่ไม่เปิดเผยข้อมูลสำคัญ
- Public API ต้องมี Rustdoc
- Async task ทุกตัวต้อง Cancel ได้และมี timeout

### TypeScript / React

- TypeScript strict mode
- ห้าม `any` เว้นแต่มี comment อธิบายและมี runtime validation
- Component ต้องมีหน้าที่เดียว
- Data จาก Native/Extension ต้อง validate ด้วย Schema
- หลีกเลี่ยง State ซ้ำซ้อนและ Derived state ที่เก็บถาวร
- UI state ต้องรองรับ Loading, Empty, Stale, Partial และ Error
- Accessibility: Keyboard navigation, visible focus, semantic roles, reduced motion

### SQL

- Migration เป็นแบบ append-only
- ห้ามแก้ Migration ที่ถูก Release แล้ว
- Query สำคัญต้องมี Index plan และ Test
- Timestamp เก็บเป็น UTC
- Monetary amount เก็บเป็น integer minor units หรือ decimal string ห้ามใช้ floating point

## 6. Test requirements

- Domain logic: Unit tests
- Storage: Migration + repository integration tests
- Adapter: Contract tests + sanitized fixtures
- Tauri commands: Integration tests
- UI: Vitest + React Testing Library
- Main workflow: Playwright E2E
- Browser Helper: Extension unit tests + Native Messaging protocol tests
- Privacy: Fixture scan ต้องยืนยันว่า Prompt/Response/Path ไม่ถูก Persist
- Release: Smoke test Installer และ Portable อย่างน้อย x64; Tier 1 ต้องมี automated build checks

## 7. File safety

- ห้ามแก้ไฟล์นอก Task โดยไม่มีเหตุผล
- ห้ามลบ Config เดิมของผู้ใช้
- ห้ามเขียนทับ Hook เดิม หากต่อ chain ไม่ได้ให้หยุดและแจ้ง
- Generated files ต้องระบุว่า Generated และห้ามแก้ด้วยมือ
- Fixture ต้องเป็นข้อมูลสังเคราะห์ ไม่มีข้อมูลจริงของผู้ใช้
- Asset ภายนอกที่จำเป็นต่อ Offline UI ต้องเก็บใน Repository พร้อม License
- ห้ามโหลด Font, Script หรือ UI asset จาก CDN ใน Runtime

## 8. Completion report

เมื่อจบ Task ให้รายงาน:

1. ไฟล์ที่เปลี่ยน
2. พฤติกรรมที่เพิ่มหรือแก้
3. Test และคำสั่งที่รัน พร้อมผล
4. ข้อจำกัดที่ยังมี
5. Security/Privacy impact
6. Commit hash หากมี
7. ยืนยันว่าไม่ได้ทำ Task ถัดไป
