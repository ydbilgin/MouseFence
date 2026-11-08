# Hook recovery

The hook handle is not liveness evidence. The guard still uses cursor movement plus the callback delta;
idle input remains inconclusive. An install failure is also actionable without subsequent movement.

On a dead verdict the production host retires the old hook thread, unregisters Raw Input, releases its
desktop binding, and transfers its engine/installer/fake-or-system cursor to a fresh message-loop thread.
Counters, toggles and the trace ring retain their shared identity. The new engine resets and seeds from
the cursor on the hook desktop. The tray also probes the cursor through that thread, because a launcher
may have left the tray on a different desktop.

Before any hook or window exists, the real installer uses OpenInputDesktop and SetThreadDesktop on the
dedicated thread. It never calls SwitchDesktop, changes a process window station, elevates, or bridges
sessions. Session 0 and noninteractive window stations are rejected. Tests inject installers whose
desktop preparation is a fake; they never bind to the real input desktop or install a real hook.

Source: the hook-recovery brief requires bounded retries and backoff. The implementation defaults are
three recovery attempts per outage, with 3 and 6 seconds between attempts (observed at the 3-second
probe). A successful installation alone does not reset this budget. Only newly delivered callbacks,
or explicit Stop/Start, re-arm it. Exhaustion leaves the last successfully installed hook in place and
logs the exhausted state; there is no busy retry loop. Unavailable desktops may therefore require an
explicit resume after the budget is exhausted. A non-answering thread gets at most the existing
5-second reply timeout per command; it is never forcibly killed or duplicated. If it cannot retire,
the recovery attempt fails rather than risking two hooks. Thread/process creation failure after
retirement is reported; a later Start cannot recreate an exited thread and requires an app restart.

Startup and recovery logs include PID, thread ID, session, window station, inherited/bound desktop,
integrity RID (not a token or user SID), Win32 failure codes, reason, attempt, backoff and exhaustion.
No logging or desktop query occurs inside the callback. Historical logs without this context cannot
prove whether a particular helper restart inherited a hidden desktop, hit UIPI, or lost its hook to a
timeout. A same-session early restart can instead exit at the single-instance mutex while the old
process is still shutting down; no second installed hook is produced by that path.

Windows contracts:

- [SetWindowsHookEx desktop scope](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexa)
- [LowLevelMouseProc message loop and timeout](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)
- [SetThreadDesktop restrictions](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setthreaddesktop)
- [OpenInputDesktop](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openinputdesktop)
