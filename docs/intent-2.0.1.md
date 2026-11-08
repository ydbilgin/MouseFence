# Intent crossing in 2.0.1

The recommended Intent model uses a 200 ms approach history and a short continued
push after contact. It is a heuristic, not a reliable reading of a person's mind.
Free/Blocked policies, game confinement and sole-exit safety precedence still apply.
Classic, Classic1k and StopThenPush remain available for comparison.

Relative mouse counts come from a message-only Raw Input receiver with INPUTSINK.
No legacy messages are disabled and no keyboard input is registered. The receiver
batches into at most 200 updates/second. The fixed 64-bucket history covers 320 ms.
Screen pixels do not build pressure. Missing/older-than-32-ms raw data holds a Push
crossing; Free routes, pause and game lock keep their existing meanings.

Approach features are angle from the edge normal, net normal travel, recent speed
(20 ms) and the ratio to the previous 40 ms speed. Speed windows end at the last
received raw sample so transport lag does not impersonate deceleration. A stable
or accelerating approach needs a small continued push. Grazing, deceleration and
short fast flicks are held. An uncertain arrival can escape through a renewed
steady push after 65 ms. Waiting alone cannot build pressure. Reversal, leaving an
attempt and changing seams invalidate contact.

| Ease | Maximum angle | Approach counts | Push counts | Minimum contact ms |
|---|---:|---:|---:|---:|
| 1 | 25.6° | 40 | 24 | 24 |
| 2 | 30.1° | 32 | 20 | 20 |
| 3 (default) | 35.0° | 24 | 16 | 16 |
| 4 | 39.4° | 18 | 12 | 12 |
| 5 | 43.5° | 12 | 8 | 8 |

Held mouse buttons lower approach/push counts by 25%; they never bypass angle or
policy checks. Device DPI and personal motion affect these counts. The slider
changes all the above thresholds together. New installations and first legacy
imports choose Intent; existing v2 choices are preserved.

## Steady push (2.0.2)

2.0.1 held many deliberate crossings for a quarter second or more: a hand that brakes
as it reaches the edge was an "uncertain arrival", and a brisk push above 12 counts/ms
(an ordinary pace on a high-DPI mouse) was a "flick". Pushing harder made it worse.

The steady-push path adds one more way through, whatever the arrival looked like:
keep pushing outward for the hold time below at an even pace, and the edge lets go.
All of these must hold, measured on raw counts after the arrival buckets:

- contact held for the hold time (no reversal of 2 counts, no pause that ends contact);
- the push runs without a pause of two silent 5 ms buckets (a pause restarts the run),
  its newest moving bucket goes outward and at least 80% of its moving buckets do;
- the last 15 ms keep at least the keep share of the pace of the run's first 15 ms,
  per moving bucket (a braking overshoot dies out, a deliberate push keeps its pace);
- post-contact pressure reaches the short-push count and the along-edge travel stays
  within the angle limit;
- recent speed is at most 30 counts/ms, so long fast game swipes stay held.

| Ease | Hold ms | Keep share |
|---|---:|---:|
| 1 | 90 | 0.85 |
| 2 | 75 | 0.85 |
| 3 (default) | 60 | 0.82 |
| 4 | 50 | 0.80 |
| 5 | 40 | 0.80 |

Tuned on the owner's own traces (300 files, ease 5): crossing time p90 fell from
524 ms to 141 ms and crossings over 150 ms from 29 to 16 of 155. Of 14 held files with
no crossing, 6 more now cross; each was followed by his real crossing 0.45-1.1 s later.
The 2.0.1 quick path already passes overshoots that arrive at full speed and brake
only after contact (320 of 720 synthetic must-hold cases); the steady path adds none.

`SteadyPushMs` in settings.json: 0 follows the slider (default), 30..=500 sets the hold
time directly, -1 turns the path off (exact 2.0.1 behaviour). The quick path, Free/Blocked
policies, game confinement and sole-exit safety precedence are unchanged.

## Local tuning data

Edge recording is opt-in. Files remain under the existing local trace folder,
bounded to 300 files / 20 MiB. A 30-second memory window supplies 1.5 seconds before
and 0.5 seconds after events. Held attempts are sampled at most twice/second.
The hook only writes a fixed SPSC queue; the tray performs file I/O. There is no
screen content, keyboard text or network upload.

Records keep the existing action (0 pass, 1 held, 2 actual guarded crossing) and
add action 3 for batched raw counts. The backwards-compatible `features` array is:

0. angle in millidegrees
1. normal speed in counts/second
2. recent/prior speed ratio in permille
3. net normal approach counts
4. post-contact pressure counts
5. contact duration in milliseconds
6. held mouse button mask
7. reason (1 unavailable/stale, 2 uncertain arrival, 3 short push, 4 grazing,
   5 deceleration/flick, 6 accepted approach, 7 renewed push, 8 policy/confinement,
   9 steady push)
8. direction ordinal (Up 1, Down 2, Left 3, Right 4)
9. raw sample age in milliseconds
10. raw dx for action 3; accepted screen x when model_state bit 128 is set
11. raw dy for action 3; accepted screen y when model_state bit 128 is set

The replay grid includes five Intent ease levels. Old records lacking raw counts
cannot establish Intent accuracy. Counterfactual classification allows the same
destination within +/-100 ms of the recorded crossing, since lighter models may
pass earlier. This is distinct from synthetic contact-to-pass latency tests.
Unmarked crossings are not ground-truth user intention; held files need human
interpretation before reporting real false-negative rates.

## Sources of the design

[Dual Monitor Tools](https://dualmonitortool.sourceforge.net/dmt_cursor.html)
motivated additional travel resistance; [DisplayFusion's mouse guide](https://www.displayfusion.com/HelpGuide/MouseManagement/)
describes why Windows catches corners when selecting corner targets.
[Ziebart et al., IUI 2012](https://www.cs.cmu.edu/~bziebart/publications/pointing-target-prediction.pdf)
motivated considering velocity and acceleration together. This implementation
does not reproduce their probabilistic learning algorithm.
[Microsoft's Raw Input documentation](https://learn.microsoft.com/en-us/windows/win32/inputdev/using-raw-input)
defines background relative mouse input. No source's accuracy claims transfer to
this heuristic. Real desk comfort and game compatibility still require use.
