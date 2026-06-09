---
phase: 3
title: Pet render
status: completed
priority: P1
effort: 3h
dependencies:
  - 2
---

# Phase 3: Pet render

## Overview
Render nhánh caption: có waiting → vẽ list các dòng waiting (chấm amber + project +
timer, cap 4 + "+N"); không waiting → caption 1 dòng như hiện tại. Window co giãn cao
theo số dòng. Tách phần vẽ caption ra module riêng (file đã ~580 dòng).

## Requirements
- Functional:
  - `waiting` không rỗng → bỏ caption 1 dòng, vẽ: tên agent (header) + mỗi dòng
    waiting `● project · {elapsed}`, tối đa 4 dòng, dư hiện dòng `+N nữa`.
  - rỗng → caption 1 dòng `● name · state` (giữ `draw_label` hiện tại, gồm cả dot màu).
  - Timer mỗi dòng đếm lại mỗi giây (tái dùng tick timer sẵn có).
- Non-functional: vẫn transparent/keep-above/sticky/drag; co giãn cả khi tăng và giảm
  số dòng (tái dùng path resize X11 trong `set_size`).

## Architecture
- Tách module: tạo `crates/agentpet/src/pet/caption.rs` chứa `draw_label`,
  `mood_caption`, `rounded_rect`, và hàm mới `draw_waiting_list(cr, w, h, name, &rows, now)`.
  `pet/mod.rs` `mod caption; use caption::*;`.
- Layout chiều cao: pet cần biết cao bao nhiêu cho list. Vì window non-resizable sized
  theo content, tính `desired_height = sprite_size + caption_block_height(rows)` và set
  qua cơ chế `set_size` (đã đẩy geometry qua X11). Giữ width = sprite_size (dòng waiting
  co chữ để vừa — tái dùng auto-shrink font đã có trong `draw_label`).
- Vẽ trong cùng `DrawingArea` (cairo) — sprite ở trên, caption-block ở dưới.
- Timer: tick timer (`TICK_MS`) hiện chỉ redraw khi frame/bob đổi. Khi có waiting,
  thêm điều kiện redraw mỗi ~1s để timer nhảy (so sánh giây nguyên trong `last_drawn` key).

## Related Code Files
- Create: `crates/agentpet/src/pet/caption.rs`
- Modify: `crates/agentpet/src/pet/mod.rs` (draw closure, height sizing, tick redraw key,
  chuyển các hàm caption sang module mới)

## Implementation Steps
1. Tạo `pet/caption.rs`, chuyển `draw_label` + `mood_caption` + `rounded_rect` sang đó
   (giữ `pub(crate)`); `pet/mod.rs` khai báo `mod caption;`.
2. Viết `caption_block_height(rows_len, has_waiting) -> i32` (1 dòng vs N dòng + header).
3. Viết `draw_waiting_list(cr, w, h, name, rows: &[WaitingRow], now)`:
   - header pill tên agent; rồi tối đa 4 pill `● project · {format_elapsed}`.
   - nếu `rows.len() > 4` → pill cuối `+{rows.len()-4} nữa`.
   - dùng amber dot (màu waiting `#f0b020`), tái dùng `rounded_rect` + auto-shrink font.
   - tái dùng `format_elapsed` (chuyển thành `pub(crate)` từ `monitor.rs` hoặc nhân bản nhỏ).
4. Trong draw closure (`pet/mod.rs`): nếu `waiting` không rỗng → `draw_waiting_list(...)`
   sau khi vẽ sprite; else → `draw_label(...)` như cũ.
5. Sizing: khi `set_waiting` đổi số dòng, tính `desired_height` và gọi nội bộ path
   resize (tách `resize_to(height)` từ logic `set_size`) để window cao vừa list.
6. Tick redraw key: thêm thành phần "giây của row đầu" để timer cập nhật ~1s/lần khi waiting.
7. `cargo build -p agentpet` + chạy thử bằng mắt (phase 4 chốt).

## Success Criteria
- [ ] Có 2 session waiting → pet hiện 2 dòng `● project · timer`, timer tăng mỗi giây.
- [ ] >4 waiting → 4 dòng + `+N nữa`.
- [ ] Hết waiting → về caption 1 dòng, window co lại đúng kích thước.
- [ ] Drag + keep-above + sticky vẫn hoạt động.
- [ ] Không pack → blob + list vẫn vẽ đúng.

## Risk Assessment
- **Co giãn window dưới XWayland**: `pet-size-setting` đã ghi nhận shrink khó dưới
  Mutter (đã có `resize_window` qua X11 + `set_default_size` nudge) — tái dùng đúng path
  đó, đừng tự nghĩ cách mới.
- **Width chật**: project name dài → dựa auto-shrink font đã có; nếu vẫn tràn, ellipsize
  project (cắt ký tự) — quyết trong lúc làm.
- File `pet/mod.rs` to: bắt buộc tách `caption.rs` để mỗi file gần ngưỡng 200 dòng.
