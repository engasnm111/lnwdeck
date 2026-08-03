# Security Policy

`inwdeck` เก็บข้อมูลแบบ Metadata-only และ local-only เป็นค่าเริ่มต้น แม้กระนั้นขอให้รายงานช่องโหว่โดยตรงผ่านช่องทางส่วนตัว ห้ามเปิดเป็น Public issue

## Reporting a vulnerability

- อย่าเปิด Public GitHub issue สำหรับช่องโหว่
- ส่งอีเมลไปยังทีมรักษาความปลอดภัยของ inwdeck พร้อม:
  - รายละเอียดของปัญหาแบบย่อ
  - ขั้นตอน Reproduce โดยใช้ Fixture สังเคราะห์เท่านั้น (ห้ามส่ง Log/ข้อมูลจริงของผู้ใช้)
  - เวอร์ชันและสภาพแวดล้อม (Windows version, architecture)
- Maintainer จะยืนยันการรับเรื่องภายใน 48 ชั่วโมงและประสานงานการแก้ไขและการเปิดเผย

## Things to include in a report

- เวกเตอร์การโจมตีที่คาด
- ผลกระทบต่อ privacy หรือการรั่วไหลของ Secret
- ผลกระทบต่อการแก้ไข Config ของผู้ใช้
- ผลกระทบต่อ Supply chain ของ Update

## Our posture

- ความลับไม่ถูก Persist ลงไฟล์ธรรมดา
- Secret ใช้ Windows Credential Manager หรือ Tauri Stronghold
- Browser Helper ใช้ Manifest V3, origin allowlist และ Native Messaging ที่ต้อง validate
- Community Adapter รันใน Wasm Sandbox แบบ deny-by-default
- Release ถูกบล็อกเมื่อมี Critical/High finding ที่ยังไม่บรรเทา
- Signature ของ Update ต้องตรวจสอบก่อนติดตั้ง

## Disclosure and credit

- Maintainer ใช้ GitHub Security Advisory สำหรับช่องโหว่ที่ยืนยันแล้ว
- ผู้รายงานจะได้รับเครดิตตามข้อตกลงก่อนเปิดเผย

## Supported versions

- v0.1: stable ตัวเดียวที่สนับสนุน; แพทช์ความปลอดภัยแบบเร่งด่วนสำหรับ v0.1
- เวอร์ชัน Alpha/Beta: โครงสร้าง database และ adapter ยังเปลี่ยนได้ รับการแก้ไขบน main