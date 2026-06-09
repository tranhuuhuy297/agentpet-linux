# Brainstorm — Pet hiển thị session waiting

Date: 2026-06-09 · Status: agreed, ready for `/ck:plan`

## Problem
Nhiều session cùng 1 agent kind gom vào 1 pet (`aggregate_by_kind`). `Working` thắng
trong aggregate → session `waiting` (cần user trả lời) bị che. Pet chỉ hiện 1 mood,
user không biết có session đang chờ. User không muốn 1-pet-1-session (quá nhiều).

## Agreed rule
> Có ≥1 session `waiting` → hiện TẤT CẢ session waiting. Không có waiting → hiện 1
> trạng thái gộp (như hiện tại).

`waiting` = việc cần user xử lý; N session chờ = N việc → phải thấy đủ. `working`/`done`
là nền → 1 trạng thái gộp đủ.

## Design

### 1. Mood priority — waiting wins
- `MoodResolver::aggregate` (`crates/agentpet-core/src/mood.rs:13`): đổi từ
  `Working > Waiting > Done` thành **`Waiting > Working > Done > Idle`**.
- Hệ quả: có waiting → sprite = waiting, tray = amber (dù còn session working). [confirmed #1]

### 2. Pet caption — nhánh theo rule
- **Có waiting:** thay caption 1 dòng bằng list các session waiting. Mỗi dòng:
  chấm amber + tên project + timer (`state_since` → elapsed). KHÔNG message. [confirmed #2]
  - Cap **4 dòng**, dư thì dòng cuối "+N nữa". [confirmed #4]
- **Không waiting:** caption 1 dòng như hiện tại (`● Claude · working`). [unchanged]

### 3. Right-click pet — giữ nguyên mở Monitor [confirmed #3]
- `attach_right_click` (`pet/mod.rs:543`) giữ `UiCommand::ShowMonitor`. Vẫn là
  fallback khi không có tray.

### 4. Data plumbing
- `UiUpdate.sessions` đã có (`snapshot.rs:13`). `sync_pets` (`ui/mod.rs:73`) lọc
  session `waiting` theo từng kind, sort theo recency, đẩy xuống pet qua method mới
  `PetWindow::set_waiting(rows)` (rows = {project, state_since}).
- Timer: tái dùng tick timer sẵn có trong pet để cập nhật elapsed mỗi giây.
- Window co giãn chiều cao theo số dòng waiting (tái dùng path resize X11 sẵn có).

## Files touched
- `crates/agentpet-core/src/mood.rs` — đảo ưu tiên aggregate + sửa tests
  (`working_wins`, `waiting_beats_done`, `by_kind_*`).
- `crates/agentpet/src/ui/mod.rs` — lọc waiting theo kind trong `sync_pets`, gọi `set_waiting`.
- `crates/agentpet/src/pet/mod.rs` — store waiting rows, render nhiều caption pill,
  co giãn chiều cao. **Lưu ý:** file đã ~580 dòng → tách phần vẽ caption ra module
  riêng (rule <200 dòng).
- (có thể) `crates/agentpet/src/snapshot.rs` — nếu cần truyền sessions vào `sync_pets`.

## Acceptance criteria
- Claude có 2 session waiting + 1 working → pet sprite=waiting, tray=amber, caption
  list 2 dòng waiting (project + timer), timer chạy mỗi giây.
- Hết waiting → pet về 1 caption gộp (working/done).
- >4 waiting → 4 dòng + "+N nữa".
- Pet vẫn kéo được, vẫn keep-above/sticky/skip-taskbar; right-click vẫn mở Monitor.
- Không có pack → blob + list vẫn chạy.

## Out of scope
- Click vào dòng để focus/đóng session.
- Itemize session working/done.
- Đổi sort của Monitor window.
- Popover right-click (đã bỏ, giữ Monitor).

## Open questions
- Có muốn đổi sort Monitor cho waiting lên đầu để đồng bộ với pet không? (hiện
  working-first) — minor, chưa chốt.
