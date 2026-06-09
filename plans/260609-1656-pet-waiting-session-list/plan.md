---
title: Pet hiển thị session waiting
description: ''
status: completed
priority: P2
branch: main
tags: []
blockedBy: []
blocks: []
created: '2026-06-09T10:02:11.376Z'
createdBy: 'ck:plan'
source: skill
---

# Pet hiển thị session waiting

## Overview

Pet gom nhiều session/agent vào 1 mood; `Working` thắng → session `waiting` (cần
user) bị che. Đổi để **ưu tiên waiting**: có ≥1 session waiting → sprite=waiting,
tray=amber, và caption liệt kê TẤT CẢ session waiting (project + timer, cap 4 +
"+N"). Không có waiting → caption 1 dòng gộp như hiện tại. Right-click giữ mở Monitor.

Brainstorm: [brainstorm-summary.md](./brainstorm-summary.md)

## Phases

| Phase | Name | Status |
|-------|------|--------|
| 1 | [Mood priority](./phase-01-mood-priority.md) | Completed |
| 2 | [Data plumbing](./phase-02-data-plumbing.md) | Completed |
| 3 | [Pet render](./phase-03-pet-render.md) | Completed |
| 4 | [Tests & verify](./phase-04-tests-verify.md) | Completed |

## Dependencies

- None. `pet-size-setting` (Done) cũng chạm `pet/mod.rs` nhưng đã hoàn tất → không chặn.
