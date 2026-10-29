# Rolx Client — User Requirements

## 1. Overview

The rolx client is a personal time-tracking companion for interacting with the RolX server. It serves the user's daily workflow: stamping work events quickly, managing bookings, writing work reports, planning days off, and getting an overview of current and past work periods.

**Design principle:** Stamping must be fast — fire-and-forget, no waiting for login or network. The client is offline-first with background sync to the RolX server.

---

## 2. Core Workflows

### 2.1 Quick Stamping (Fire-and-Forget)

The user needs to record work events instantly, without friction:

| Event | Description |
|---|---|
| **Stamp in** | Start of a work session |
| **Stamp out** | End of a work session |
| **Break start** | Starting a break (lunch, coffee, etc.) |
| **Break end** | Returning from a break |
| **Switch project** | Changing the active activity/project mid-session |

**Requirements:**

- [ ] Stamping must complete locally in under 1 second — no waiting for authentication or server round-trips.
- [ ] The client MUST authenticate and cache credentials on first use, then auto-refresh tokens silently. No login prompt during stamping.
- [ ] Stamp events are stored locally immediately and synced to the server asynchronously in the background.
- [ ] If the device is offline, stamping still works — events queue for later sync.
- [ ] The user should be able to trigger stamping with minimal interaction: a single button press (TUI) or short CLI command (`rolx stamp in`).
- [ ] When stamping out, if no break is currently active, the system automatically stamps out of the break first.
- [ ] When stamping in, if still stamped in from before, the previous session is auto-closed with current time.

### 2.2 Project Booking

Every work entry must be booked to a project (activity). The booking can happen at stamp time or later.

**Requirements:**

- [ ] The user can select an activity when stamping in (optional — not required for fast stamping).
- [ ] If no activity is selected at stamp time, the entry is marked as "unassigned" and flagged for later review.
- [ ] Unassigned entries appear prominently in a pending-review view.
- [ ] The user can assign/change the project for existing entries in batch or individually.
- [ ] Activity selection uses the server's activity list (cached locally after first fetch).
- [ ] Favourite activities are surfaced at the top of the selection list.

### 2.3 Work Reports

Work reports describe what was done during a time slot. They can be written before, during, or after work.

**Requirements:**

- [ ] The user can write a work report independently of any stamping — "dump from mind" mode.
- [ ] Standalone (unlinked) reports are stored and later assignable to existing unassigned entries.
- [ ] Reports can be written in advance as plans ("plan to fix bug X") and modified after the fact.
- [ ] Each work entry supports a free-text comment/report field.
- [ ] The user can link an existing standalone report to a specific entry.
- [ ] If a project requires reports (configurable per activity), missing reports are flagged in overviews.

### 2.4 Modifying Bookings

Existing bookings must be editable after the fact (within edit-lock constraints).

**Requirements:**

- [ ] **Change project/activity** for any existing entry.
- [ ] **Update work report/comment** for any existing entry.
- [ ] **Adjust timestamps** — move start/end times of entries.
- [ ] When adjusting a timestamp, the client MUST detect and warn about:
  - **Overlaps:** If moving an entry's end time past the next entry's start time (or vice versa). The user is prompted to resolve: shift adjacent entry, truncate, or merge.
  - **Gaps:** If moving an entry away from an adjacent one, creating a gap. The user is prompted to fill the gap (new break? unbooked time?) or leave it as-is.
- [ ] All modifications respect the server's edit-lock date — entries before the lock date are read-only.
- [ ] Modifications are synced to the server asynchronously after local validation.

---

## 3. Days Off Planning

The user needs to plan and track various types of time off.

**Requirements:**

- [ ] **Vacation days:** Plan and book vacation dates. Show remaining available vacation days (from balance API).
- [ ] **Sickness leave:** Mark days as sick leave with optional reason/date range.
- [ ] **Bank holidays:** Displayed automatically from server's YearInfo; cannot be modified.
- [ ] **Overtime compensation:** Plan future days off paid out of accumulated overtime balance.
- [ ] **Other legally protected leave:** Support for military service, family reasons, etc. (mapped to `PaidLeaveType.Other` with reason).
- [ ] Planned time off is factored into weekly/daily target calculations.
- [ ] The user can view a calendar-style overview of planned days off.

---

## 4. Daily Overview

A dashboard-like view for the current day showing all relevant information at a glance.

**Requirements:**

### 4.1 Time Summary

- [ ] Total hours booked today (sum of all entries).
- [ ] Hours booked per project/activity, with visual breakdown.
- [ ] Current session status: stamped in/out, time since last stamp.
- [ ] Remaining time to daily target work hours (from current time if stamped in, or from end of workday).
- [ ] Overtime counter: accumulated overtime balance (from server), updated periodically.

### 4.2 Weekly Overview

- [ ] Hours booked this week vs. weekly target.
- [ ] Weekly target is calculated from YearInfo monthly work times, adjusted for:
  - Planned days off (vacation, sickness, etc.) in the current week.
  - Bank holidays in the current week.
  - Part-time factor from user's `PartTimeSetting`.
- [ ] Remaining hours to weekly target (or surplus overtime accumulated this week).
- [ ] Running total of all accumulated overtime (from balance API).

### 4.3 Work Report Status

- [ ] List of work reports per project for the current day/week.
- [ ] Warning indicator if a project that requires reports has missing or unassigned entries.
- [ ] Quick link from warning to the pending-review view for that project.

### 4.4 Timetable

- [ ] Visual timeline of today's entries (start time, end time, activity, duration).
- [ ] Breaks shown as gaps in the timeline with labels.
- [ ] Color-coded by project/activity for quick scanning.
- [ ] Current position indicator if stamped in.

---

## 5. Monthly and Yearly Analysis

*(To be defined in a later session.)*

Planned capabilities:

- Monthly summary of hours, overtime delta, vacation balance, billable vs non-billable time.
- Yearly overview with cumulative statistics.
- Trend visualization (hours per week, overtime accumulation, vacation usage).
- Export capabilities for reporting to management or personal records.

---

## 6. Non-Functional Requirements

### 6.1 Offline-First

- [ ] All core operations (stamping, viewing, editing) work offline.
- [ ] Local database stores all work records, plans, and standalone reports.
- [ ] Background sync reconciles with the server when connectivity is available.
- [ ] Conflict resolution: server's edit-lock date is authoritative; local changes before lock are synced, after lock are rejected with user notification.

### 6.2 Authentication & Security

- [ ] Initial authentication via Google Sign-In (JWT flow) or API key (for CLI).
- [ ] JWT tokens are cached and auto-refreshed silently.
- [ ] Credentials stored securely in OS-native credential storage (keyring on Linux, Keychain on macOS, Keystore on Android).
- [ ] No work data leaves the device without explicit sync.

### 6.3 Performance

- [ ] Stamping response time: < 1 second local operation.
- [ ] Daily overview loads in < 2 seconds (from local cache).
- [ ] Background sync is non-blocking and does not interfere with UI responsiveness.

---

## 7. Interfaces (To Be Defined)

The following interfaces will be designed in subsequent sessions:

- **CLI:** Command-line interface for quick stamping, querying records, and managing bookings from terminal/scripts.
- **Desktop TUI:** Terminal-based user interface with dashboard views, calendar, and interactive editing — suitable for daily use on a desktop/laptop terminal.
- *(Future: Mobile app, Web/PWA)*
