---
phase: 4
title: Tests & verify
status: completed
priority: P2
effort: 1h
dependencies:
  - 3
---

# Phase 4: Tests & verify

## Overview
Khoá hành vi bằng unit test (core) + build/clippy sạch + kiểm mắt trên màn hình thật
(cần agent + pet pack, không tự động hoá được dưới headless).

## Requirements
- Functional: toàn bộ test pass, không regress.
- Non-functional: clippy không warning mới; cập nhật changelog + bump version.

## Architecture
- Core logic (`aggregate`) test ở phase 1. Layer GTK không unit-test được trực tiếp →
  tách hàm thuần để test: `waiting_rows_for` (sort/lọc) và `caption_block_height` /
  cap "+N" (đặt trong `caption.rs`, test thuần không cần GTK context).

## Related Code Files
- Modify: `crates/agentpet-core/src/mood.rs` (tests — phase 1 đã làm, xác nhận)
- Modify: `crates/agentpet/src/pet/caption.rs` (tests thuần cho cap/+N + chiều cao)
- Modify: `crates/agentpet/src/ui/mod.rs` (test `waiting_rows_for` lọc/sort)
- Modify: `docs/project-changelog.md` + `crates/agentpet/Cargo.toml` (bump)

## Implementation Steps
1. Unit test `waiting_rows_for`: chỉ lấy session waiting đúng kind, sort recency desc,
   project = basename.
2. Unit test logic cap: 3 rows → 3 pill, 0 "+N"; 7 rows → 4 pill + "+3 nữa".
3. `cargo test -p agentpet-core -p agentpet` — toàn bộ pass.
4. `cargo clippy --all-targets` — không warning mới.
5. Kiểm mắt (manual): mở 2 agent, để 1 cái waiting (vd Claude hỏi permission) →
   xác nhận sprite waiting, tray amber, caption list dòng waiting + timer chạy; xử lý
   xong → về caption gộp, window co lại. (Ghi nhận giống `pet-size-setting`: shrink dưới
   Mutter cần kiểm tay.)
6. Cập nhật `docs/project-changelog.md` (mục mới) + bump `crates/agentpet/Cargo.toml`
   (0.9.0 → 0.10.0) cho lần release sau.

## Success Criteria
- [ ] `cargo test -p agentpet-core -p agentpet` pass.
- [ ] `cargo clippy --all-targets` sạch.
- [ ] Kiểm mắt: waiting list + timer + co giãn + drag OK.
- [ ] Changelog + version bump xong.

## Risk Assessment
- Render/sizing không kiểm được headless → bắt buộc kiểm mắt trước khi đóng. Không
  đánh dấu done nếu chưa chạy thật.
