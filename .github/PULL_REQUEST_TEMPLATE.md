## Purpose

_อธิบายหนึ่งจุดประสงค์ของ PR นี้_

## Task ต้นทาง

- [ ] Task: _Task ID/name จาก Implementation plan_
- [ ] ไม่ได้ทำ Task อื่นนอกเหนือจากที่แจ้ง

## สิ่งที่เปลี่ยน

- ไฟล์ที่เพิ่ม/แก้:
- พฤติกรรมที่เปลี่ยน:

## Test / Quality gates

- [ ] `pnpm check` ผ่าน
- [ ] `pnpm test` ผ่าน
- [ ] `cargo test --workspace --all-features` ผ่าน
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` ผ่าน
- [ ] `cargo fmt --check` ผ่าน
- [ ] Privacy scan ผ่าน (ถ้าเกี่ยวข้อง)

## Security / Privacy impact

_ระบุผลต่อ Metadata-only, Secret, Permission หรือ Hook อย่างชัดเจน_

## Dependency ใหม่

_รายชื่อ dependency, เหตุผล, License_

## Checklist

- [ ] ไม่มี Prompt/Response/Path/Secret ถูก Persist
- [ ] Fixture เป็นข้อมูลสังเคราะห์เท่านั้น
- [ ] Documentation ตรงกับโค้ด
- [ ] Commit เล็กและอ่านง่าย