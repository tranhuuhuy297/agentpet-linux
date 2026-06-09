---
phase: 2
title: Data plumbing
status: completed
priority: P1
effort: 1.5h
dependencies:
  - 1
---

# Phase 2: Data plumbing

## Overview
Đẩy danh sách session `waiting` theo từng agent kind xuống đúng `PetWindow`, để
phase 3 render. `UiUpdate.sessions` đã có sẵn — chỉ cần lọc + chuyển dạng gọn.

## Requirements
- Functional: mỗi pet (theo kind) nhận danh sách dòng waiting của riêng kind đó,
  sort theo recency (mới nhất trước), kèm `state_since` để tính timer.
- Non-functional: tránh leak `AgentSession` đầy đủ vào layer pet — dùng struct gọn.

## Architecture
- Struct mới (đặt ở `pet/mod.rs`): `WaitingRow { project: String, state_since: f64 }`.
  `project` rút gọn = basename path (tái dùng cùng cách `monitor.rs:155` làm), fallback id.
- `Ui::apply` (`ui/mod.rs:58`) đang gọi `sync_pets(&update.moods)`. Đổi để truyền
  cả `&update.sessions` (hoặc lọc sẵn map kind→Vec<WaitingRow>).
- `sync_pets`: với mỗi kind active, lọc `sessions` theo `agent_kind==kind &&
  state==Waiting`, sort `updated_at` desc, map sang `WaitingRow`, gọi
  `pet.set_waiting(rows)` (cả khi tạo mới lẫn update).

## Related Code Files
- Modify: `crates/agentpet/src/ui/mod.rs` (`apply`, `sync_pets`)
- Modify: `crates/agentpet/src/pet/mod.rs` (thêm `WaitingRow` + `set_waiting`)
- (Không cần đổi `snapshot.rs` — `UiUpdate.sessions` đã đủ.)

## Implementation Steps
1. `pet/mod.rs`: thêm `pub struct WaitingRow { pub project: String, pub state_since: f64 }`.
2. `pet/mod.rs`: thêm field `waiting: Rc<RefCell<Vec<WaitingRow>>>` vào `PetWindow`,
   khởi tạo rỗng; thêm `pub fn set_waiting(&self, rows: Vec<WaitingRow>)` ghi vào RefCell
   rồi `self.area.queue_draw()`.
3. `ui/mod.rs`: helper `fn waiting_rows_for(kind, sessions: &[AgentSession]) -> Vec<WaitingRow>`
   (lọc + sort desc theo `updated_at` + basename project).
4. `ui/mod.rs`: đổi `sync_pets(&self, moods, sessions)` nhận thêm sessions; trong vòng
   tạo/update gọi `pet.set_waiting(waiting_rows_for(*kind, sessions))`.
5. `ui/mod.rs`: `apply` truyền `&update.sessions` vào `sync_pets`.
6. `cargo build -p agentpet` sạch (chưa cần render — phase 3 dùng dữ liệu này).

## Success Criteria
- [ ] `set_waiting` lưu được rows và trigger redraw.
- [ ] `sync_pets` đẩy đúng waiting-rows theo kind (Claude pet chỉ nhận session Claude).
- [ ] `cargo build -p agentpet` không lỗi.

## Risk Assessment
- `state_since` là Unix time; timer tính `now - state_since` ở render (phase 3) — đảm
  bảo dùng cùng `crate::unix_now()` như monitor để nhất quán.
