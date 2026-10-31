# RolX client apps

Custom client apps for the RolX time tracking server, used to book and review working time.

## Language

**Booked time**:
Time recorded in entries on activities. Absences are not booked time.
_Avoid_: Worked time

**Chart**:
A line chart of the booked time on one activity over the days of one month. The chart of all activities also shows the target time.
_Avoid_: Graph, plot

**Pie chart**:
A chart of the share each activity has in the booked time of one month. Small activities are combined into one "Other" slice.
_Avoid_: Breakdown, distribution

**Share**:
The part of a month's booked time that falls on one activity.

**Daily hours**:
The measure of a chart that shows the booked time of each day.

**Running total**:
The measure of a chart that shows the booked time accumulated from the first day of the month up to each day.
_Avoid_: Cumulative hours

**Target time**:
The time a user is expected to book on a day: the working time the server expects, less any absence. It is zero on weekends and holidays.
_Avoid_: Nominal time, Target state, Goal

**Vacation left**:
The vacation a user has not taken yet as of today, including planned vacation.
_Avoid_: Available vacation

**Planned vacation**:
Vacation booked on days after today. It is part of the vacation left, not on top of it.

**Stamp**:
A started measurement of working time that has not been stamped out yet. Stamping out turns it into entries.
_Avoid_: Timer, clock-in

**Local copy**:
The copy of the user's records and reference data kept on the machine, so the app works without a connection.
_Avoid_: Cache

**Pending change**:
A change to a day that is saved locally and has not been uploaded to the server yet.
_Avoid_: Offline entry, queued entry

**Outbox**:
The list of pending changes waiting to be uploaded.
_Avoid_: Queue

**Syncing**:
Uploading the outbox to the server and refreshing the local copy from it.

**Rejected**:
A pending change the server refused. It stays in the outbox with the server's message until the user fixes or discards it.

**Conflict**:
A pending change whose day was changed on the server after the pending change was made. The user decides what happens to it.
