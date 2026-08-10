# คู่มือแหล่งโควต้าและการตั้งค่า Provider

เอกสารนี้อธิบายว่า lnwdeck ดึง “การใช้งาน” และ “โควต้า” จากที่ใด รวมถึง
ขั้นตอนสำหรับเครื่องที่ยังไม่มี provider ติดตั้งหรือยังไม่ได้ล็อกอิน

## อ่านก่อนเริ่ม

การใช้งานกับโควต้าเป็นคนละข้อมูลกัน:

- `Usage` คือจำนวน token/request ที่อ่านได้จาก log หรือฐานข้อมูลในเครื่อง
- `Quota` คือ limit ที่ provider ประกาศผ่าน API, dashboard หรือข้อมูล balance
  ของ provider เอง
- ถ้าแหล่งข้อมูลไม่มี limit จริง lnwdeck จะแสดง usage-only หรือ not supported
  และจะไม่สร้างเปอร์เซ็นต์จาก rolling window เอง
- เครื่องที่ไม่มี provider จะแสดง `ไม่มีการเชื่อมต่อ` และไม่ทำให้ refresh ทั้งรอบ
  ล้มเหลว

การตั้งค่าควรทำจากหน้า Settings ของ lnwdeck เมื่อมีฟอร์มให้ใช้ ค่า secret
จะถูกเก็บใน Windows Credential Manager และไม่ถูกส่งเข้า UI, SQLite, log หรือ
export

## OpenCode Go: ตั้งค่าบนเครื่องอื่น

OpenCode Go เป็นกรณีพิเศษ เพราะ quota ไม่ได้อยู่ใน local SQLite แต่ประกาศอยู่ที่
workspace dashboard ของ OpenCode ดังนั้น “แต่ละเครื่อง” ที่ต้องการดู quota ต้องมี
คู่ค่าของตัวเอง:

- `OPENCODE_GO_WORKSPACE_ID`
- `OPENCODE_GO_AUTH_COOKIE`

ทั้งสองค่าต้องมีพร้อมกัน lnwdeck จะไม่ใช้ค่าจากเครื่องแรกโดยอัตโนมัติ และจะไม่
เดา workspace หรือแสดง 100% แทนข้อมูลจริง

### วิธีที่แนะนำ: ใช้ Settings

1. เปิด lnwdeck แล้วไปที่ `Settings` → `Provider credentials` → `OpenCode Go`
2. เปิดหน้า OpenCode ใน browser และล็อกอินด้วยบัญชีที่ต้องการใช้บนเครื่องนี้
3. นำ workspace id ของ workspace ที่มี Go plan มาใส่ในช่อง
   `OPENCODE_GO_WORKSPACE_ID`
4. นำค่า session cookie ชื่อ `auth` มาใส่ในช่อง
   `OPENCODE_GO_AUTH_COOKIE` โดยใส่ได้ทั้งค่า raw เช่น `ey...` หรือทั้งคู่แบบ
   `auth=ey...`
5. กด Save แล้วกด Refresh providers
6. ปิดและเปิด lnwdeck ใหม่ถ้า widget เดิมยังแสดงข้อมูล cache

ผลลัพธ์ที่คาดหวัง:

- เมื่อ dashboard ตอบกลับและมีข้อมูล: OpenCode Go แสดงหน้าต่าง quota พร้อม
  เปอร์เซ็นต์และเวลาคืนโควต้า
- เมื่อยังไม่ได้ตั้งค่า: แสดง `ไม่มีการเชื่อมต่อ`/`ต้องตั้งค่า` และไม่มีแถบ
  เปอร์เซ็นต์
- เมื่อ cookie หมดอายุ: แสดง `การยืนยันตัวตนหมดอายุ` ให้เข้าสู่ระบบใหม่
- ถ้า HTML ของ dashboard เปลี่ยนรูปแบบ: แสดงข้อมูลไม่พร้อมชั่วคราวโดยไม่เดา
  ค่าใหม่

cookie นี้เทียบเท่ากับ session credential ห้าม commit, ส่งในแชต, ใส่ใน
`.env`, screenshot หรือ command history ถ้าใช้ Settings แล้วให้ลบค่าจาก
environment ของเครื่องนั้นเพื่อไม่ให้มีสำเนาสองที่

### วิธีสำรอง: ตั้ง environment variable ด้วย PowerShell

เปิด PowerShell ของ user คนที่จะรัน lnwdeck แล้วใช้ค่าจริงของเครื่องนั้น:

```powershell
$env:OPENCODE_GO_WORKSPACE_ID = "ใส่_workspace_id_ของคุณ"
$env:OPENCODE_GO_AUTH_COOKIE = "ใส่ค่า_auth_cookie_ของคุณ"
Start-Process "lnwdeck.exe"
```

ค่ารูปแบบนี้อยู่เฉพาะ process/PowerShell หน้าต่างนั้น เหมาะสำหรับทดสอบก่อน

ถ้าต้องการให้คงอยู่สำหรับการเปิดครั้งต่อไป ใช้ `setx` ทีละตัว แล้วเปิด PowerShell
ใหม่ก่อนเปิดโปรแกรม:

```powershell
setx OPENCODE_GO_WORKSPACE_ID "ใส่_workspace_id_ของคุณ"
setx OPENCODE_GO_AUTH_COOKIE "ใส่ค่า_auth_cookie_ของคุณ"
```

ข้อควรระวัง:

- `setx` ไม่เปลี่ยนค่าใน PowerShell หน้าต่างปัจจุบัน ต้องเปิดหน้าต่างใหม่
- ต้องตั้งทั้งสองตัว ถ้าขาดตัวใดตัวหนึ่งจะถือว่า `NOT_CONFIGURED`
- environment variable จะอยู่ใน user profile ของ Windows และอาจถูกอ่านได้โดย
  process อื่นภายใต้ user เดียวกัน จึงควรใช้ Credential Manager เป็นวิธีถาวร
- ห้ามใช้ `echo $env:OPENCODE_GO_AUTH_COOKIE`, ห้ามใส่ลง `.env` และห้าม paste
  ค่า secret ลง issue/commit
- หลังจาก Save ผ่าน Settings แล้ว ลบ environment variable ได้ด้วย:

```powershell
[Environment]::SetEnvironmentVariable("OPENCODE_GO_WORKSPACE_ID", $null, "User")
[Environment]::SetEnvironmentVariable("OPENCODE_GO_AUTH_COOKIE", $null, "User")
```

ถ้าต้องการกลับไปใช้ Credential Manager ต้องลบ environment ทั้งสองตัวพร้อมกัน
เพราะ environment จะมี precedence เหนือค่าที่เก็บไว้ใน Credential Manager

แหล่งข้อมูลที่ใช้เทียบพฤติกรรม OpenCode Go คือ [TokenTracker]
([https://github.com/xiufengsun/TokenTracker](https://github.com/xiufengsun/TokenTracker)):
dashboard เป็นแหล่ง authoritative และ local estimate ต้องเป็น opt-in เท่านั้น
lnwdeck จึงไม่ใช้ local token total เป็น quota โดยอัตโนมัติ

## Provider matrix ปัจจุบัน

| Provider | Usage ในเครื่อง | แหล่ง quota ที่อนุญาต | ต้องตั้งค่าอะไร |
|---|---|---|---|
| Claude | session JSONL | Anthropic OAuth usage API | รัน `claude` login บนเครื่องนั้น |
| OpenAI Codex | session JSONL | ChatGPT `/wham/usage` และ reset-credit endpoint; local rate snapshot เป็น fallback ที่ provider ประกาศ | รัน `codex login` |
| Cursor | account API/local state | Cursor account usage summary API | ล็อกอิน Cursor บนเครื่องนั้น |
| Gemini | session/log | Gemini Code Assist quota API; รายละเอียดราย window จาก Antigravity IDE Language Server เมื่อ IDE เปิดอยู่ | ล็อกอิน Antigravity IDE หรือ Gemini CLI บนเครื่องนั้น |
| OpenCode Go | OpenCode SQLite | `https://opencode.ai/workspace/{workspace}/go` | คู่ env หรือ Settings ตามขั้นตอนด้านบน |
| ZCode | ZCode SQLite | Z.AI/BigModel monitor API หรือ `billing/balance` log ที่ ZCode เขียนเอง | coding-plan credential ของ ZCode ถ้ามี |
| Kimi Code | `wire.jsonl` | `https://api.kimi.com/coding/v1/usages` และ OAuth refresh | ล็อกอิน Kimi; รองรับ `KIMI_HOME`/`KIMI_CODE_HOME` |
| Grok | ไม่มี usage channel ในตัว | xAI rate-limit headers/API | ใส่ key ใน Settings |
| OpenRouter | ไม่มี usage channel ในตัว | OpenRouter credits/limits API | ใส่ key ใน Settings |
| Ollama | ไม่มี usage channel ในตัว | local API probe; แสดง unlimited เฉพาะเมื่อ API ตอบ | เปิด Ollama ที่เครื่องนั้น |
| GitHub Copilot | local artifact | ยังไม่มีแหล่ง quota ที่รับรองใน adapter นี้ | usage ได้; quota แสดง not supported |
| Kiro AI | local artifact/sidecar | ยังไม่มีแหล่ง quota ที่รับรองใน adapter นี้ | usage ได้; quota แสดง not supported |
| Z.AI (GLM) | local log | ยังไม่มี limit API ที่ผูกกับ adapter นี้ | usage ได้; quota แสดง not supported |
| Kilo CLI | local artifact | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |
| Kilo Code | local artifact | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |
| Mimo Code | local SQLite | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |
| Roo Code | IDE task history | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |
| CodeBuddy | IDE artifact | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |
| WorkBuddy | IDE artifact | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |
| pi | session artifact | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |
| oh-my-pi | notify/session artifact | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |
| Hermes | session artifact | ไม่มี limit ที่ provider ประกาศให้ adapter ใช้ | usage ได้; quota แสดง not supported |

คำว่า `not supported` ในตารางเป็นผลลัพธ์ที่ตั้งใจ: ไม่ใช่ refresh failure และ
ไม่ควรถูกแทนด้วย 0%, 100% หรือ limit ที่คำนวณจากจำนวน token

## หลายบัญชีและ App / CMD / WSL

lnwdeck แยกบัญชีด้วย fingerprint ที่สร้างจาก account identity ของ provider
และกุญแจเฉพาะฐานข้อมูลเครื่องนั้น โดยไม่เก็บ token, cookie, API key หรือ
account id ดิบลง SQLite, UI, log หรือ diagnostics

- App กับ CMD บน Windows ที่ใช้ credential/source เดียวกันจะถูกรวมเป็นบัญชีเดียว
- ถ้า provider เปิดเผย account id, workspace id หรือ user subject ระบบจะใช้ค่า
  นั้นเพื่อให้ token ที่หมุนใหม่ของบัญชีเดิมยังรวมกันได้
- ถ้า fingerprint ต่างกัน ระบบจะเก็บ quota และ event แยกกัน และ widget/หน้า
  Providers จะแสดง `Account 1`, `Account 2` (หรือคำแปลของภาษาที่เลือก)
- provider ที่เปิดเผยเพียง token ชั่วคราวอาจได้ fingerprint ใหม่เมื่อ provider
  หมุน token; ให้ล็อกอินผ่าน credential source เดิมเพื่อหลีกเลี่ยงการแยกบัญชี

WSL เป็น environment แยกจาก Windows: environment variable ใน WSL ไม่ถูกส่งเข้า
process ของ lnwdeck ที่รันบน Windows และ Credential Manager ของ Windows ก็ไม่ใช่
ไฟล์ credential ของ WSL โดยอัตโนมัติ ถ้าต้องการให้บัญชีเดียวกันถูกรวม ให้ทำตาม
ขั้นตอนนี้บน Windows ด้วย:

1. ล็อกอิน provider ใน WSL และตรวจสอบว่าเป็นบัญชีที่ต้องการ
2. ล็อกอิน provider เดียวกันใน App/CMD Windows หรือคัดลอกเฉพาะขั้นตอน login ที่
   provider รองรับไปยัง Windows (ห้ามคัดลอก token ลงเอกสารหรือ command history)
3. สำหรับ OpenCode Go ให้ใส่ `OPENCODE_GO_WORKSPACE_ID` และ
   `OPENCODE_GO_AUTH_COOKIE` ใน Settings ของ lnwdeck บน Windows หรือกำหนดใน
   Windows PowerShell ตามขั้นตอนด้านบน ไม่ใช่กำหนดเฉพาะใน WSL
4. ปิด lnwdeck ให้หมดจาก tray แล้วเปิดใหม่ จากนั้นกด Refresh providers
5. ตรวจสอบว่า quota ที่เป็นบัญชีเดียวกันอยู่ใน card เดียว; ถ้าขึ้น Account 1/2
   ให้ตรวจว่า provider ใช้คนละ account id/workspace หรือ credential คนละชุดจริง

การอ่านข้อมูลเป็น passive read-only: lnwdeck ไม่สั่ง login แทนผู้ใช้ ไม่เขียนทับ
ไฟล์ credential และไม่ส่งข้อมูลจาก WSL หรือ Windows ไป cloud ของ lnwdeck

## การแก้ปัญหาโดยดูสถานะ

### `ไม่มีการเชื่อมต่อ`

หมายถึง source ของ provider ไม่พบในเครื่อง เช่น ยังไม่ได้ติดตั้ง CLI, ยังไม่เคย
เปิด provider หรือ profile อยู่คนละตำแหน่ง ให้ติดตั้ง/ล็อกอิน provider นั้นแล้ว
กด refresh ใหม่

### `ต้องตั้งค่า` หรือ `ยังไม่ได้ตั้งค่า`

หมายถึงพบ provider หรือพบ source แล้ว แต่ credential ที่จำเป็นยังไม่มี เช่น
OpenCode Go มี local database แต่ยังไม่มี workspace/cookie คู่กัน ให้ทำตาม
ส่วนตั้งค่าของ provider นั้น

### `การยืนยันตัวตนหมดอายุ`

ให้ล็อกอิน provider ใหม่ (`claude`, `codex`, Kimi หรือ OpenCode ใน browser)
แล้ว refresh อีกครั้ง lnwdeck จะไม่เขียนทับ credential file ของ provider

### Provider บางรายไม่มีโควต้า แต่ refresh สำเร็จ

เป็นพฤติกรรมปกติสำหรับ provider ที่เป็น usage-only หรือ not supported การ
refresh จะเก็บผลของ provider ที่มีอยู่ต่อไป และไม่แสดงข้อความรวมว่า
“รีเฟรชผู้ให้บริการบางรายไม่สำเร็จ” เพียงเพราะเครื่องนี้ไม่มี provider เหล่านั้น

## สำหรับผู้พัฒนา adapter

ก่อนประกาศว่า quota รองรับ ต้องมีหลักฐานจาก provider API/dashboard หรือข้อมูล
balance ที่ provider เขียนเอง พร้อม fixture ที่ตรวจ:

1. response ไม่มี window ต้องได้ `Ok(None)` หรือ error code ที่ sanitize แล้ว
2. ห้ามแปลง usage-only local scan เป็น `from_percent`
3. ห้ามสร้าง `with_limit` ถ้า provider ไม่ได้ส่ง limit จริง
4. ต้องแยก `NOT_CONFIGURED`, `SOURCE_UNAVAILABLE`, `AUTH_EXPIRED` และ
   `SOURCE_SCHEMA_MISMATCH`
5. เปลี่ยนแหล่งข้อมูลต้องอัปเดต descriptor, contract matrix, UI state และเอกสาร

[TokenTracker]: https://github.com/xiufengsun/TokenTracker
