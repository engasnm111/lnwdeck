# Contributing to inwdeck

ขอบคุณที่สนใจช่วยพัฒนา inwdeck ขอให้อ่านเอกสารต่อไปนี้ก่อนเริ่ม:

- `AGENTS.md` — กฎบังคับสำหรับ AI agent และ Contributor
- `docs/00_PROJECT_CHARTER.md` — เป้าหมาย ขอบเขต และการตัดสินใจที่ล็อกแล้ว
- `docs/02_SYSTEM_ARCHITECTURE.md` — สถาปัตยกรรมและขอบเขตของ Layer
- `docs/05_SECURITY_PRIVACY.md` — ข้อกำหนด Privacy/Secret/Hook/Browser
- `docs/08_TESTING_QA.md` — Test strategy และ Quality gates

## หลักการทำงาน

- ทำงานครั้งละ หนึ่ง Task จาก Implementation plan
- Commit ต้อง เล็ก อ่านง่าย และมีจุดประสงค์เดียว
- ห้ามแก้ Requirement เอง; ห้ามเริ่ม Task ถัดไปก่อน Task ปัจจุบัน review ผ่าน
- ทำงาน Inline; ห้ามสร้าง Subagent เว้นแต่ได้รับอนุมัติ

## Privacy-first rule

- เก็บเฉพาะ Metadata เท่านั้น
- ห้ามเพิ่ม Prompt, Response, Source code, File name หรือ Absolute path
- Data ใหม่ทุกตัวต้องผ่าน Privacy guard ที่ fail-closed
- Fixture ต้องเป็นข้อมูลสังเคราะห์ ไม่มีข้อมูลจริงของผู้ใช้

## วิธีตรวจ Pull Request

การตรวจสอบที่ต้องผ่านก่อน merge:

- `pnpm check`
- `pnpm test`
- `cargo test --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- หากเกี่ยวข้องกับ Provider: run contract suite + privacy scan

## Community / Verified adapter

- Built-in: maintainer review + full contract tests
- Verified community: source สาธารณะ, checksum, review, contract tests พาส
- Unverified community: ต้องแสดง warning และ manual install
- รันโค้ด Community ผ่าน Wasm sandbox; ห้าม native DLL plugin

## Dependency

- Dependency ใหม่ต้องอธิบายเหตุผล ตรวจ License และ ปรับขนาดผลกระทบ
- ห้ามโหลด Font / Script / UI asset จาก CDN ใน Runtime

## แก้ Bug

- Reproduce ก่อน ใส่ทีละครั้ง ไม่พังการทำงานของ Provider อื่น
- Error ต้อง typed และ context ไม่เปิดเผยข้อมูลอ่อนไหว
- ไม่มี `unwrap()`/`expect()` ใน Production path

## Commit message

ใช้ Conventional commits เช่น `feat:`, `fix:`, `chore:`, `docs:`, `test:`, `build:`

ตัวอย่างจากแผนงาน:

```text
chore: establish inwdeck workspace
feat: expose secure desktop application commands
```

## ขั้นตอนเปิด PR

1. ทำงานบน branch แยกจาก `main`/`dev`
2. รัน Quality gates ให้ผ่าน
3. เปิด PR โดยใช้เทมเพลตจาก `.github/PULL_REQUEST_TEMPLATE.md`
4. มี Reviewer review; แล้วแต่อนุมัติและ merge