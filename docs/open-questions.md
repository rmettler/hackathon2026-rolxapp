# Open questions, decisions and manual checks

Collected while implementing `docs/plan.md`. Open questions never block the work: a default is
applied and noted here, and all questions are asked at the end.

## Open questions

| # | Question | Default used |
|---|---|---|
| Q1 | App display name and Android bundle id | "RolX", `ch.cudos.rolxapp` |
| Q2 | UI language | English |
| Q3 | Push the `add-dioxus` branch to GitHub? | no |
| Q4 | Scope beyond CLI parity (stamping, offline-first, bookings from `docs/user-requirements.md`) | out of scope |
| Q5 | RISC-V box: distro, libc, access (SSH?) | static musl binary, no access assumed |
| Q6 | LAN access to the headless host: own domain + TLS, Tailscale, or SSH tunnel only | localhost / SSH tunnel |
| Q7 | Android minimum SDK version | dx template default |
| Q8 | Windows bundle format | dx default |
| Q9 | Hackathon deadline / priority if time runs out | milestone order as in the plan |

## Decisions taken

| Milestone | Decision |
|---|---|
| M0 | Toolchain pinned to stable with the wasm32 and Android targets (`rust-toolchain.toml`). |
| M0 | Android env vars (`ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME`) set at user level; the JDK is Android Studio's bundled JBR. |

## Manual verification checklist

Checks that need a human, real credentials or real devices. Each entry says what to do and what
to expect.

- [ ] (filled in as milestones are completed)

## Review findings accepted without a fix

Minor findings from the per-milestone reviews that were deliberately not fixed.

- (none yet)
