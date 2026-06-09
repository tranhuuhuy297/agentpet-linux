---
phase: 1
title: Mood priority
status: completed
priority: P1
effort: 1h
dependencies: []
---

# Phase 1: Mood priority

## Overview
Đảo ưu tiên trong `MoodResolver::aggregate` để `Waiting` thắng `Working`. Đây là
core logic thuần (GTK-free), có test sẵn → sửa test trước rồi đổi code.

## Requirements
- Functional: có ≥1 session `Waiting` → mood gộp = `Waiting` (dù còn `Working`).
  Thứ tự mới: `Waiting > Working > Done > Idle`.
- Non-functional: không đổi signature; `aggregate_by_kind` vẫn drop kind `Idle`.

## Architecture
- `aggregate(&[AgentSession]) -> PetMood`: kiểm tra `Waiting` TRƯỚC `Working`.
- `aggregate_by_kind` không đổi (gọi lại `aggregate`).
- Hệ quả lan tới: sprite (phase 3) + tray. Tray `waiting` count (`snapshot.rs:27`)
  đã đếm độc lập theo `AgentState::Waiting` → không cần đổi; chỉ mood gộp đổi.

## Related Code Files
- Modify: `crates/agentpet-core/src/mood.rs` (hàm `aggregate` + tests)

## Implementation Steps
1. Trong `aggregate`, đổi thứ tự: check `AgentState::Waiting` trước `AgentState::Working`:
   ```rust
   if sessions.iter().any(|s| s.state == AgentState::Waiting) { return PetMood::Waiting; }
   if sessions.iter().any(|s| s.state == AgentState::Working) { return PetMood::Working; }
   if sessions.iter().any(|s| s.state == AgentState::Done)    { return PetMood::Done; }
   PetMood::Idle
   ```
2. Sửa test `working_wins` (đang assert Working khi có Working+Waiting+Done) →
   đổi tên `waiting_beats_working` + assert `PetMood::Waiting`.
3. Giữ `waiting_beats_done`, `by_kind_gives_each_agent_its_own_mood`,
   `by_kind_aggregates_within_a_kind` (kiểm lại: hai Claude waiting+working → giờ
   ra `Waiting`, sửa assert tương ứng), `registered_is_not_working`, `done_only`.
4. Cập nhật doc-comment `aggregate` mô tả thứ tự mới (waiting cần user nên ưu tiên).
5. `cargo test -p agentpet-core` xanh.

## Success Criteria
- [ ] `aggregate` trả `Waiting` khi tập có cả Working và Waiting.
- [ ] Test phản ánh waiting-wins thay cho `working_wins`, pass.
- [ ] `cargo test -p agentpet-core` pass toàn bộ.

## Risk Assessment
- Đổi ngữ nghĩa mood: user đã xác nhận (#1) — tray amber + sprite waiting khi có
  waiting là đúng ý. Không rủi ro ẩn.
