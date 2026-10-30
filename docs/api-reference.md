# RolX Server API Reference

> **Scope:** Endpoints accessible via Bearer token by any active user (`User` role and above).
> **Base path:** `https://{server}/api/v1`
> **Date format:** ISO 8601 (`yyyy-MM-dd`), unless noted otherwise.

---

## 1. Authentication

### 1.1 Get Sign-In Info

Retrieves server sign-in configuration (Google client ID and installation ID). No authentication required.

- **Method:** `GET`
- **Path:** `/api/v1/SignIn/info`
- **Auth:** None

**Response `200 OK` — `Info`:**

| Field | Type | Description |
|---|---|---|
| `googleClientId` | `string` | Google Sign-In client ID to use |
| `installationId` | `string` | RolX installation identifier |

### 1.2 Sign In

Authenticates via Google ID token. Returns a JWT bearer token.

- **Method:** `POST`
- **Path:** `/api/v1/SignIn`
- **Auth:** None
- **Content-Type:** `application/json`
- **Body — `SignInData`:**

| Field | Type | Required | Description |
|---|---|---|---|
| `googleIdToken` | `string` | Yes | Google OAuth ID token |

**Response `200 OK` — `Approval`:**

| Field | Type | Description |
|---|---|---|
| `user` | `User` | Authenticated user object |
| `bearerToken` | `string` | HS256 JWT (default 1-day lifetime) |
| `expires` | `string` (ISO 8601 datetime) | Token expiry timestamp |

**Response `401 Unauthorized`** — invalid Google token.

### 1.3 Extend Token

Refreshes the JWT before expiry.

- **Method:** `GET`
- **Path:** `/api/v1/SignIn/extend`
- **Auth:** Bearer token required
- **Response `200 OK` — `Approval`** (same shape as sign-in, with updated `bearerToken` and `expires`).

---

## 2. Work Records

All endpoints below enforce the `ActiveUser` policy (JWT must be valid and user must be active during the relevant date). Users can only access their own records unless they hold `Supervisor` role or above.

### 2.1 Get Record Range

Retrieves work records for a user within a half-open date range `(begin..end]`.

- **Method:** `GET`
- **Path:** `/api/v1/WorkRecord/{userId}/range/{beginDate}..{endDate}`
- **Auth:** Bearer token
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `userId` | `string` (GUID) | User identifier |
| `beginDate` | `string` (ISO `yyyy-MM-dd`) | Inclusive start date |
| `endDate` | `string` (ISO `yyyy-MM-dd`) | Exclusive end date |

**Response `200 OK`** — `Record[]`

**Response `404 Not Found`** — invalid date format.
**Response `403 Forbidden`** — unauthorized (not own user and below Supervisor role).

### 2.2 Update Record

Updates a single work record for a specific date.

- **Method:** `PUT`
- **Path:** `/api/v1/WorkRecord/{userId}/{date}`
- **Auth:** Bearer token
- **Content-Type:** `application/json`
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `userId` | `string` (GUID) | User identifier |
| `date` | `string` (ISO `yyyy-MM-dd`) | Record date |

**Body — `Record`:** (see §4.1)

**Response `204 No Content`** on success.
**Response `400 Bad Request`** — date/user ID mismatch.
**Response `403 Forbidden`** — user inactive at date or unauthorized.

### 2.3 Bulk Update Records

Updates multiple records atomically.

- **Method:** `PUT`
- **Path:** `/api/v1/WorkRecord/{userId}/bulkupdate`
- **Auth:** Bearer token
- **Content-Type:** `application/json`
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `userId` | `string` (GUID) | User identifier |

**Body — `BulkRecord`:**

| Field | Type | Description |
|---|---|---|
| `records` | `Record[]` | Records to update |

**Response `204 No Content`** on success.
**Response `400 Bad Request`** — bad date or user ID mismatch.
**Response `403 Forbidden`** — user inactive at any date or unauthorized.

---

## 3. Balance

### 3.1 Get Balance by Date

Retrieves the time balance for the authenticated user at a given date.

- **Method:** `GET`
- **Path:** `/api/v1/Balance/{date}`
- **Auth:** Bearer token
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `date` | `string` (ISO `yyyy-MM-dd`) | Date to query |

**Response `200 OK` — `Balance`:** (see §4.2)
**Response `404 Not Found`** — invalid date format.

---

## 4. Edit Lock

### 4.1 Get Edit Lock

Retrieves the current edit lock date. Records before this date cannot be modified.

- **Method:** `GET`
- **Path:** `/api/v1/edit-lock`
- **Auth:** Bearer token
- **Response `200 OK` — `EditLock`:**

| Field | Type | Description |
|---|---|---|
| `date` | `string` (ISO `yyyy-MM-dd`) | Lock date (modifications before this are prohibited) |

---

## 5. Year Info

### 5.1 Get Year Info

Retrieves holiday and monthly work time schedule for a calendar year.

- **Method:** `GET`
- **Path:** `/api/v1/YearInfo/{year}`
- **Auth:** Bearer token (also accepts API key)
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `year` | `integer` | Calendar year (e.g., `2026`) |

**Response `200 OK` — `YearInfo`:** (see §4.3)

---

## 6. Activities

### 6.1 Get All Activities

Lists all activities, optionally filtered by end date.

- **Method:** `GET`
- **Path:** `/api/v1/Activity`
- **Auth:** Bearer token
- **Query params:**

| Param | Type | Description |
|---|---|---|
| `unlessEndedBeforeDate` | `string?` (ISO `yyyy-MM-dd`) | If provided, excludes activities ended before this date |

**Response `200 OK` — `Activity[]`:** (see §4.4)

### 6.2 Get Suitable Activities

Returns activities suitable for a user on a specific date.

- **Method:** `GET`
- **Path:** `/api/v1/Activity/suitable/{userId}/{date}`
- **Auth:** Bearer token
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `userId` | `string` (GUID) | User identifier |
| `date` | `string` (ISO `yyyy-MM-dd`) | Date |

**Response `200 OK` — `Activity[]`**
**Response `404 Not Found`** — invalid date.
**Response `403 Forbidden`** — unauthorized.

### 6.3 Get Activity by ID

Returns a single activity with current times spent.

- **Method:** `GET`
- **Path:** `/api/v1/Activity/{id}`
- **Auth:** Bearer token
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `id` | `integer` | Activity identifier |

**Response `200 OK` — `Activity`**
**Response `404 Not Found`** — activity not found.

---

## 7. Subprojects

### 7.1 Get All Subprojects

Lists all subprojects in shallow form.

- **Method:** `GET`
- **Path:** `/api/v1/Subproject`
- **Auth:** Bearer token
- **Response `200 OK` — `SubprojectShallow[]`:** (see §4.5)

### 7.2 Get Subproject by ID

Returns full subproject detail including activities.

- **Method:** `GET`
- **Path:** `/api/v1/Subproject/{id}`
- **Auth:** Bearer token
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `id` | `integer` | Subproject identifier |

**Response `200 OK` — `Subproject`:** (see §4.6)
**Response `404 Not Found`** — subproject not found.

---

## 8. Billability

### 8.1 Get All Billability Types

Lists all active billability types.

- **Method:** `GET`
- **Path:** `/api/v1/Billability`
- **Auth:** Bearer token
- **Response `200 OK` — `Billability[]`:** (see §4.7)

---

## 9. Favourite Activities

### 9.1 Get Favourite Activities

Returns the authenticated user's favourite activities.

- **Method:** `GET`
- **Path:** `/api/v1/activity/favourite`
- **Auth:** Bearer token
- **Response `200 OK` — `Activity[]`**

### 9.2 Add Favourite

Adds an activity to the user's favourites.

- **Method:** `PUT`
- **Path:** `/api/v1/activity/favourite`
- **Auth:** Bearer token
- **Content-Type:** `application/json`
- **Body — `Activity`:** (activity to add; only `Id` is used)
- **Response `204 No Content`** on success.

### 9.3 Remove Favourite

Removes an activity from the user's favourites.

- **Method:** `DELETE`
- **Path:** `/api/v1/activity/favourite`
- **Auth:** Bearer token
- **Content-Type:** `application/json`
- **Body — `Activity`:** (activity to remove; only `Id` is used)
- **Response `204 No Content`** on success.

---

## 10. User Reports

### 10.1 Get Monthly User Report

Returns the monthly report for a specified user and month.

- **Method:** `GET`
- **Path:** `/api/v1/user-report/{userId}/month/{month}`
- **Auth:** Bearer token
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `userId` | `string` (GUID) | User identifier |
| `month` | `string` (ISO `yyyy-MM`) | Month |

**Response `200 OK` — `UserMonthReport`:** (see §4.18)
**Response `404 Not Found`** — invalid month format or no report found.
**Response `403 Forbidden`** — unauthorized.

---

## 11. User Lookup

### 11.1 Get User by ID

Retrieves a user by identifier. Requires `Supervisor` role or above.

- **Method:** `GET`
- **Path:** `/api/v1/User/{id}`
- **Auth:** Bearer token (role: Supervisor+)
- **Path params:**

| Param | Type | Description |
|---|---|---|
| `id` | `string` (GUID) | User identifier |

**Response `200 OK` — `User`:** (see §4.8)
**Response `403 Forbidden`** — insufficient role.

---

## Data Types

### §4.1 `Record`

A work record for a single day.

| Field | Type | Description |
|---|---|---|
| `date` | `string` (ISO `yyyy-MM-dd`) | Record date |
| `userId` | `string` (GUID) | User identifier |
| `dayType` | `DayType` | Type of day (see §5.1) |
| `dayName` | `string` | Name of the day (e.g. "Monday") |
| `nominalWorkTime` | `integer` (long) | Nominal work time in seconds |
| `paidLeaveType` | `PaidLeaveType?` | Type of paid leave (see §5.2) |
| `paidLeaveReason` | `string?` | Reason for paid leave |
| `entries` | `RecordEntry[]` | Time entries for the day |

### §4.2 `Balance`

Time balance at a given date.

| Field | Type | Description |
|---|---|---|
| `byDate` | `string` (ISO `yyyy-MM-dd`) | Date of this balance |
| `overtime` | `integer` (seconds) | Accumulated overtime |
| `vacationAvailable` | `integer` (seconds) | Available vacation time |
| `vacationAvailableDays` | `number?` | Available vacation in days |
| `vacationPlanned` | `integer` (seconds) | Planned vacation time |
| `vacationPlannedDays` | `number?` | Planned vacation in days |

### §4.3 `YearInfo`

Schedule information for a calendar year.

| Field | Type | Description |
|---|---|---|
| `holidays` | `Holiday[]` | Public holidays |
| `monthlyWorkTimes` | `MonthlyWorkTime[]` | Work days/hours per month |

### §4.4 `Activity`

A work activity within a subproject.

| Field | Type | Description |
|---|---|---|
| `id` | `integer` | Activity identifier |
| `number` | `integer` | Activity number |
| `name` | `string` | Activity name |
| `startDate` | `string` (ISO `yyyy-MM-dd`) | Start date |
| `endDate` | `string?` (ISO `yyyy-MM-dd`) | End date (null = ongoing) |
| `billabilityId` | `integer` | Billability type ID |
| `billabilityName` | `string` | Billability type name |
| `isBillable` | `boolean` | Whether the activity is billable |
| `isOverBudget` | `boolean` | Whether budget is exceeded |
| `isOverPlanned` | `boolean` | Whether planned hours are exceeded |
| `budget` | `integer` (long) | Budget in seconds |
| `planned` | `integer` (long) | Planned hours in seconds |
| `actual` | `integer` (long) | Actual hours in seconds |
| `budgetConsumedFraction` | `number` | Fraction of budget consumed |
| `plannedConsumedFraction` | `number` | Fraction of planned consumed |
| `projectName` | `string` | Parent project name |
| `subprojectName` | `string` | Parent subproject name |
| `customerName` | `string` | Customer name |
| `fullNumber` | `string` | Full activity number (e.g. "PROJ-001-ACT") |
| `fullName` | `string` | Full activity name |
| `allSubprojectNames` | `string` | All subproject names |

### §4.5 `SubprojectShallow`

Lightweight subproject representation (used in list views).

| Field | Type | Description |
|---|---|---|
| `id` | `integer` | Subproject identifier |
| `fullNumber` | `string` | Full subproject number |
| `customerName` | `string` | Customer name |
| `projectName` | `string` | Parent project name |
| `name` | `string` | Subproject name |
| `managerName` | `string` | Manager name |
| `deputyManagerName` | `string?` | Deputy manager name |
| `isArchitectRequired` | `boolean?` | Whether an architect is required |
| `architectName` | `string?` | Architect name |
| `isClosed` | `boolean` | Whether the subproject is closed |

### §4.6 `Subproject`

Full subproject detail, including its activities. Extends `SubprojectShallow`.

| Field | Type | Description |
|---|---|---|
| *(inherited from `SubprojectShallow`)* | | |
| `fullName` | `string` | Full subproject name |
| `startDate` | `string?` (ISO `yyyy-MM-dd`) | Start date |
| `projectNumber` | `integer` | Project number |
| `number` | `integer` | Subproject number |
| `managerId` | `string?` (GUID) | Manager user ID |
| `deputyManagerId` | `string?` (GUID) | Deputy manager user ID |
| `architectId` | `string?` (GUID) | Architect user ID |
| `budget` | `integer` (long) | Budget in seconds |
| `planned` | `integer` (long) | Planned hours in seconds |
| `actual` | `integer` (long) | Actual hours in seconds |
| `budgetConsumedFraction` | `number` | Fraction of budget consumed |
| `plannedConsumedFraction` | `number` | Fraction of planned consumed |
| `isOverBudget` | `boolean` | Whether budget is exceeded |
| `isOverPlanned` | `boolean` | Whether planned hours are exceeded |
| `isClosed` | `boolean` | Whether the subproject is closed |
| `activities` | `Activity[]` | Activities within this subproject |

### §4.7 `Billability`

A billability type (e.g. billable engineering, non-billable).

| Field | Type | Description |
|---|---|---|
| `id` | `integer` | Billability type ID |
| `name` | `string` | Billability name |
| `isBillable` | `boolean` | Whether billable |
| `sortingWeight` | `integer` | Display sort order |
| `inactive` | `boolean` | Whether inactive (excluded from lists) |

### §4.8 `User`

A user in the system.

| Field | Type | Description |
|---|---|---|
| `id` | `string` (GUID) | User identifier |
| `firstName` | `string` | First name |
| `lastName` | `string` | Last name |
| `email` | `string` | Email address |
| `avatarUrl` | `string` | Avatar URL |
| `role` | `Role` | User role (see §5.3) |
| `entryDate` | `string?` (ISO `yyyy-MM-dd`) | Employment start date |
| `leavingDate` | `string?` (ISO `yyyy-MM-dd`) | Employment end date |
| `isConfirmed` | `boolean` | Whether the user is confirmed |
| `partTimeSettings` | `PartTimeSetting[]` | Part-time settings |
| `balanceCorrections` | `BalanceCorrection[]` | Balance correction history |
| `vacationDaysSettings` | `VacationDaysSetting[]` | Vacation days settings |
| `targetBillabilitySettings` | `TargetBillabilitySetting[]` | Target billability settings |
| `positionSettings` | `PositionSetting[]` | Position settings |

### §4.9 `Approval`

Result of a successful sign-in.

| Field | Type | Description |
|---|---|---|
| `user` | `User` | Authenticated user |
| `bearerToken` | `string` | HS256 JWT |
| `expires` | `string` (ISO 8601 datetime) | Token expiry |

### §4.10 `RecordEntry`

A single time entry within a `Record`.

| Field | Type | Description |
|---|---|---|
| `activityId` | `integer` | Activity identifier |
| `fullActivityNumber` | `string` | Full activity number |
| `duration` | `integer` (long) | Duration in seconds |
| `begin` | `integer?` | Start time in seconds since midnight |
| `pause` | `integer?` | Pause duration in seconds |
| `comment` | `string?` | Entry comment |

### §4.11 `Holiday`

A public holiday entry.

| Field | Type | Description |
|---|---|---|
| `name` | `string` | Holiday name |
| `date` | `string` (ISO `yyyy-MM-dd`) | Holiday date |

### §4.12 `MonthlyWorkTime`

Work days and hours for a month.

| Field | Type | Description |
|---|---|---|
| `month` | `string` (ISO `yyyy-MM`) | Month |
| `days` | `integer` | Number of work days |
| `hours` | `integer` | Work hours in seconds |

### §4.13 `PartTimeSetting`

A part-time factor setting for a user.

| Field | Type | Description |
|---|---|---|
| `startDate` | `string` (ISO `yyyy-MM-dd`) | Effective start date |
| `factor` | `number` (double) | Part-time factor (e.g. 0.5 for 50%) |

### §4.14 `BalanceCorrection`

A correction to a user's balance.

| Field | Type | Description |
|---|---|---|
| `date` | `string` (ISO `yyyy-MM-dd`) | Correction date |
| `overtime` | `integer` (seconds) | Overtime adjustment |
| `vacation` | `integer` (seconds) | Vacation adjustment |
| `reason` | `string` | Reason for correction |

### §4.15 `VacationDaysSetting`

Vacation days allocation setting.

| Field | Type | Description |
|---|---|---|
| `startDate` | `string` (ISO `yyyy-MM-dd`) | Effective start date |
| `vacationDays` | `integer` | Allocated vacation days |

### §4.16 `TargetBillabilitySetting`

Target billability factor setting.

| Field | Type | Description |
|---|---|---|
| `startDate` | `string` (ISO `yyyy-MM-dd`) | Effective start date |
| `factor` | `number` (double) | Target billability factor |

### §4.17 `PositionSetting`

User position setting.

| Field | Type | Description |
|---|---|---|
| `startDate` | `string` (ISO `yyyy-MM-dd`) | Effective start date |
| `position` | `Position` | User position (see §5.4) |

### §4.18 `UserMonthReport`

Monthly report for a single user.

| Field | Type | Description |
|---|---|---|
| `month` | `string` (ISO `yyyy-MM`) | Report month |
| `user` | `User` | User object |
| `partTimeSettings` | `PartTimeSetting[]` | Part-time settings |
| `balanceCorrections` | `BalanceCorrection[]` | Balance corrections |
| `vacationDaysSettings` | `VacationDaysSetting[]` | Vacation days settings |
| `targetBillabilitySettings` | `TargetBillabilitySetting[]` | Target billability settings |
| `positionSettings` | `PositionSetting[]` | Position settings |
| `overtime` | `integer` (seconds) | Total overtime |
| `overtimeDelta` | `integer` (seconds) | Overtime change from previous month |
| `vacation` | `integer` (seconds) | Total vacation time |
| `vacationDays` | `number?` | Vacation in days |
| `vacationDelta` | `integer` (seconds) | Vacation change from previous month |
| `vacationDeltaDays` | `number?` | Vacation days change |
| `billableTime` | `integer` (seconds) | Total billable time |
| `nonBillableTime` | `integer` (seconds) | Total non-billable time |
| `leaveTime` | `integer` (seconds) | Total leave time |
| `workItemGroups` | `WorkItemGroup[]?` | Grouped work items (see §4.19) |

---


### §4.19 `WorkItemGroup`

A named group of work items within a monthly report.

| Field | Type | Description |
|---|---|---|
| `name` | `string` | Group name |
| `items` | `WorkItem[]` | Work items in this group |

### §4.20 `WorkItem`

A single work item within a `WorkItemGroup`.

| Field | Type | Description |
|---|---|---|
| `name` | `string` | Work item name |
| `duration` | `integer` (seconds) | Duration of the work item |

## Enums

### §5.1 `DayType`

| Value | Description |
|---|---|
| `Workday` | A regular workday |
| `Weekend` | A weekend day |
| `Holiday` | A public holiday |

### §5.2 `PaidLeaveType`

| Value | Description |
|---|---|
| `Vacation` | Vacation / time off |
| `Sickness` | Sickness / medical absence |
| `MilitaryService` | Military, community, or civil defense service |
| `Other` | Other legally protected leave (requires a reason comment) |

### §5.3 `Role`

| Value | Numeric | Description |
|---|---|---|
| `User` | 1 | Regular user |
| `Supervisor` | 2 | Can view other users' records |
| `Backoffice` | 100 | Administration role |
| `Administrator` | 1000 | Full privileges |

### §5.4 `Position`

| Value | Numeric | Description |
|---|---|---|
| `Unassigned` | 0 | No position assigned |
| `Junior` | 10 | Cudos Trail participant |
| `Extern` | 50 | External employee |
| `Engineer` | 100 | Engineer |
| `Administration` | 1000 | Backoffice / administration |

---

## Authentication Notes

- **Bearer token:** Include as `Authorization: Bearer <jwt>` header.
- **JWT algorithm:** HS256 (HMAC-SHA256, signed with server-side symmetric secret).
- **Default lifetime:** 1 day. Extend via `GET /api/v1/SignIn/extend`.
- **`ActiveUser` policy:** The JWT must be valid AND the user must be active during the relevant date (between `entryDate` and `leavingDate`).
- **User isolation:** A `User` can only access their own records. `Supervisor` and above can access any user's records.
