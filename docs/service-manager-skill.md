# ติดตั้ง Service Manager skill

Skill นี้สอน Codex/Claude ใช้ MCP และ proxy ของ Service Manager: สร้างหรือเปิดโปรเจกต์ได้ทุกโฟลเดอร์ที่ OS อนุญาตตั้งแต่ v0.7.0 โดยไม่เพิ่ม folder grant, หา project/service ที่ถูกต้อง, Run/Stop/Restart, อ่าน log ต่อด้วย cursor, เพิ่ม/แก้ command ด้วย revision และแยกข้อความใน log ออกจากคำสั่งของผู้ใช้

Skill ไม่ได้ติดตั้ง MCP connection, สร้าง token, login AI หรือเพิ่มสิทธิ์ให้เอง ต้องเปิดแอปและสร้าง token ใน AI Connections ก่อน ตาม [คู่มือ AI](ai-connections.md)

## ตำแหน่งไฟล์

ต้นฉบับอยู่ใน `skills/service-manager/`:

```text
service-manager/
  SKILL.md
  agents/openai.yaml
  references/contract.md
```

ติดตั้งแบบส่วนตัวทั้งเครื่องโดยคัดลอกทั้งโฟลเดอร์ไปที่:

- Codex: `~/.agents/skills/service-manager/`
- Claude Code: `~/.claude/skills/service-manager/`

สำหรับ Codex รุ่นหรือ environment ที่กำหนด skills root ต่างออกไป ให้ใช้ root ของ environment นั้น และอย่าลงชื่อเดียวกันซ้ำหลาย root

ถ้าต้องการใช้เฉพาะ repository ให้คัดลอกไป `.agents/skills/service-manager/` สำหรับ Codex หรือ `.claude/skills/service-manager/` สำหรับ Claude Code ใน repository เป้าหมาย เลือก scope เดียวต่อ client เพื่อไม่ให้รายการซ้ำ

ไม่ควรทับ skill ชื่อเดียวกันที่มีอยู่ก่อน ให้ตรวจเนื้อหาและสำรองไว้ก่อนอัปเดต ชุดนี้ไม่มี credential หรือ path ส่วนตัวของ proxy อยู่ในไฟล์

## เรียกใช้งาน

หลังติดตั้งให้เปิด chat/session ใหม่ หรือ reload client หากยังไม่พบ skill

Codex:

```text
ใช้ $service-manager รัน backend ของโปรเจกต์ที่เปิดอยู่ แล้วอ่าน log 15 วินาที
```

Claude Code:

```text
/service-manager รัน backend แล้วอ่าน log ต่อจาก cursor ล่าสุด
```

ตัวอย่างงานอื่น:

- “ใช้ service-manager สร้างโปรเจกต์ demo ในโฟลเดอร์หลักที่อนุญาต ยังไม่เพิ่มหรือรัน service”
- “ใช้ service-manager เปิดโฟลเดอร์ backend เข้าแอป โดยใช้ root ที่อนุญาต”
- “ใช้ service-manager เช็กสถานะ frontend แล้วอ่านเฉพาะ error ที่เกี่ยวข้อง โดยไม่ restart”
- “ใช้ service-manager เพิ่ม service worker ด้วยคำสั่งที่กำหนด บันทึกอย่างเดียว ยังไม่รัน”
- “ใช้ service-manager หยุด backend แล้วตรวจว่าหยุดสำเร็จ”

ถ้า MCP ยังไม่ปรากฏ ให้ส่ง absolute path ของ Proxy script จากหน้า AI Connections พร้อม service ID ให้ AI proxy รองรับเฉพาะ list/run/logs และไม่หยุด service เมื่อครบเวลาอ่าน log

ตั้งแต่ v0.6.0 skill ครอบคลุม `inspect_ports`, `list_runs`, `read_run` และ `diagnose_service` ด้วย เป็นการอ่านหลักฐานเท่านั้น ไม่เรียกโมเดลหรือแก้ command อัตโนมัติ ประวัติเก็บสูงสุด 100 รอบรวม/20 รอบต่อ service และ log 64 KiB/500 records ต่อรอบที่จบ ส่วน diagnostic ปัจจุบันจำกัด 32 KiB/100 records การปิดข้อมูลลับเป็น best effort ต้องตรวจ context ก่อนส่งต่อ ปุ่ม Kill port อยู่ในแอปและต้องยืนยัน ไม่มี MCP kill tool

## ตรวจหลังติดตั้ง

ให้ client อ่าน skill และบอกวิธีใช้ `logs --cursor ... --run ... --follow` ต่อจากการ capture ครั้งแรก โดยไม่เริ่ม service ซ้ำ จากนั้นทดสอบกับ service ชั่วคราวที่ตั้งใจเปิดใช้เท่านั้น ตรวจสถานะและ runId ในแอปว่าเป็นชุดเดียวกัน

การพบไฟล์ skill ไม่ได้แปลว่า MCP เชื่อมต่อแล้ว ตั้งแต่แอป v0.5.0 token ที่ใช้ได้มีสิทธิ์อ่าน/รัน/หยุด/แก้ command ทุก service รวมที่เพิ่มภายหลัง โดยไม่เลือกทีละโปรเจกต์ ตั้งแต่ v0.7.0 การสร้างหรือเปิดโปรเจกต์ใช้ได้ทุกโฟลเดอร์ที่ OS อนุญาต โดยไม่ตั้ง parent-folder grant เพิ่ม token เดิมใช้ต่อได้ หาก app offline หรือ token revoked ต้องแก้ connection ก่อน ห้ามใช้ skill ข้าม trust หรืออ่าน credentials

เอกสารรูปแบบและ discovery ที่ตรวจเมื่อ 2026-10-16: [OpenAI official skills guide](https://learn.chatgpt.com/docs/build-skills), [Claude Code skills](https://code.claude.com/docs/en/skills)
