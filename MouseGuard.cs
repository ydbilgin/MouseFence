using System.Runtime.InteropServices;
using System.Threading;

namespace MouseFence;

/// <summary>
/// Installs the low-level mouse hook and applies <see cref="GuardCore"/>'s decision: a one-way barrier
/// above the bottom row of monitors. See <see cref="GuardCore"/> for the (tested) decision logic.
/// </summary>
public sealed class MouseGuard : IDisposable
{
    private IntPtr _hook = IntPtr.Zero;
    private readonly Native.HookProc _proc;   // kept alive so the GC never collects the callback
    private readonly GuardCore _core = new();

    // Liveness probe state (see HookHealth): a counter the callback stamps, plus the cursor position at the
    // previous probe. Both are only ever read/written from the UI thread (the hook is dispatched on the
    // installing thread's queue), but the counter is interlocked so it stays correct if that ever changes.
    private long _callbacks;
    private long _callbacksAtLastProbe;
    private Native.POINT _lastProbePoint;
    private bool _haveProbePoint;
    private bool _running;

    /// <summary>
    /// True while the guard is MEANT to be running (Start called, Stop not). This is the pause state the tray
    /// reflects — it is deliberately NOT the hook handle, because Windows can drop the hook without telling us.
    /// Ask <see cref="HookInstalled"/> for whether a hook is actually in place.
    /// </summary>
    public bool Enabled => _running;

    /// <summary>True while a hook handle is actually installed. False + <see cref="Enabled"/> = the fence is DOWN.</summary>
    public bool HookInstalled => _hook != IntPtr.Zero;

    /// <summary>Master toggle: when true the configured crossing gates are active; when false all crossing is blocked.</summary>
    public bool GateOpen { get; set; }

    /// <summary>Side-containment toggle: when true a soft barrier blocks accidental drift out of the MAIN screen
    /// left/right (a deliberate horizontal push still crosses); when false the side is free. The horizontal mirror
    /// of the up-barrier's feel (<see cref="GateOpen"/>).</summary>
    public bool SideContain { get; set; }

    /// <summary>Game mode: confine the cursor to whichever monitor it is currently on.</summary>
    public bool Confine
    {
        get => _core.Confine;
        set => _core.Confine = value;
    }

    /// <summary>When true a deliberate upward push is required to cross; when false any upward move crosses.</summary>
    public bool DeliberateCross
    {
        get => _core.DeliberateCross;
        set => _core.DeliberateCross = value;
    }

    /// <summary>Side-barrier sensitivity: the min horizontal px in one move to count as a deliberate side crossing
    /// (higher = a firmer side barrier). Independent of the up-barrier so it can be tuned on its own.</summary>
    public int SideCrossMin
    {
        get => _core.SideCrossMin;
        set => _core.SideCrossMin = value;
    }

    /// <summary>Side-barrier vertical slack: how much vertical wobble a deliberate side push may carry beyond its
    /// horizontal component before it stops counting as a sideways crossing.</summary>
    public int SideCrossSlack
    {
        get => _core.SideCrossSlack;
        set => _core.SideCrossSlack = value;
    }

    /// <summary>Opt-in: clamp a descent's exit X back onto the linked bottom monitor (default off).</summary>
    public bool DescentRouting
    {
        get => _core.DescentRouting;
        set => _core.DescentRouting = value;
    }

    public MouseGuard() => _proc = HookCallback;

    /// <summary>Configure the barrier from the top monitors, the allowed crossing gates, and all monitor rects (for game mode).
    /// <paramref name="forbidden"/> are excluded-display bounds the cursor is walled out of (headless/dummy screens).</summary>
    public void Configure(IEnumerable<Native.RECT> blocked, IEnumerable<(int Min, int Max)> gates,
                          IEnumerable<Native.RECT> allMonitors,
                          IEnumerable<RuntimeTransition> transitions,
                          IEnumerable<((int L, int T, int R, int B) Top, (int L, int T, int R, int B) Landing)> descentRoutes,
                          IEnumerable<Native.RECT> forbidden,
                          bool hasSides, Native.RECT mainBounds)
    {
        var rects = blocked.ToList();
        _core.HasTop = rects.Count > 0;
        _core.BarrierY = _core.HasTop ? rects.Max(r => r.Bottom) : 0;
        _core.Gates = gates.Where(g => g.Max > g.Min).ToList();
        _core.Monitors = allMonitors.Select(r => (r.Left, r.Top, r.Right, r.Bottom)).ToList();
        // Schema v4 carries the legacy sole-up safety escape on its compiled transition. The cut F-A1 runtime graph
        // scan stays removed; no separate mutable SafetyGates list is needed in the hook path.
        _core.SafetyGates.Clear();
        _core.Transitions = (transitions ?? Enumerable.Empty<RuntimeTransition>()).ToArray();
        _core.DescentRoutes = descentRoutes.ToList();
        _core.Forbidden = forbidden.Select(r => (r.Left, r.Top, r.Right, r.Bottom))
                                   .Where(r => r.Right > r.Left && r.Bottom > r.Top).ToList();
        _core.HasSides = hasSides;
        _core.MainL = mainBounds.Left; _core.MainR = mainBounds.Right;
        _core.MainT = mainBounds.Top; _core.MainB = mainBounds.Bottom;
        // Re-baseline: a live reconfigure (display layout change) can move the cursor's coordinate from the old top
        // into a newly positioned bottom/side screen while OnTop was true. Dropping the stale state forces the next
        // move through the !HaveLast branch, which re-derives OnTop from the fresh cursor position vs the new barrier.
        _core.Reset();
    }

    public void Start()
    {
        _running = true;
        ResetProbe();
        if (_hook != IntPtr.Zero) return;
        _core.Reset();
        _hook = Native.SetWindowsHookEx(Native.WH_MOUSE_LL, _proc, Native.GetModuleHandle(null), 0);
    }

    public void Stop()
    {
        _running = false;
        ResetProbe();
        if (_hook == IntPtr.Zero) return;
        Native.UnhookWindowsHookEx(_hook);
        _hook = IntPtr.Zero;
    }

    /// <summary>
    /// One liveness probe. Windows silently unhooks a low-level hook whose callback overruns
    /// LowLevelHooksTimeout and leaves our handle intact, so the fence can go down while everything still
    /// *looks* installed. Call this on a timer: a <see cref="HookHealthVerdict.Dead"/> verdict has already
    /// attempted a reinstall by the time it returns — check <see cref="HookInstalled"/> for whether it took.
    /// </summary>
    public HookHealthVerdict CheckHealth()
    {
        if (!_running) return HookHealthVerdict.Inconclusive;

        long seen = Interlocked.Read(ref _callbacks);
        long delta = seen - _callbacksAtLastProbe;
        _callbacksAtLastProbe = seen;

        bool moved = false;
        if (Native.GetCursorPos(out var now))
        {
            // The first probe after Start has no baseline: record it and claim no movement, so a reinstall is
            // never triggered by an unknown-vs-known comparison.
            moved = _haveProbePoint && (now.X != _lastProbePoint.X || now.Y != _lastProbePoint.Y);
            _lastProbePoint = now;
            _haveProbePoint = true;
        }

        var verdict = HookHealth.Evaluate(moved, delta);
        if (verdict == HookHealthVerdict.Dead)
            Reinstall();

        return verdict;
    }

    /// <summary>Tear down whatever handle we hold and install a fresh hook. Safe when nothing is installed.</summary>
    private void Reinstall()
    {
        if (_hook != IntPtr.Zero)
        {
            Native.UnhookWindowsHookEx(_hook);
            _hook = IntPtr.Zero;
        }

        // Moves were missed while the hook was down, so the last-position state is stale: drop it and let the
        // next move re-derive the side of the barrier the cursor is on (same reasoning as Configure's re-baseline).
        _core.Reset();
        _hook = Native.SetWindowsHookEx(Native.WH_MOUSE_LL, _proc, Native.GetModuleHandle(null), 0);
    }

    private void ResetProbe()
    {
        _callbacksAtLastProbe = Interlocked.Read(ref _callbacks);
        _haveProbePoint = false;
    }

    private IntPtr HookCallback(int nCode, IntPtr wParam, IntPtr lParam)
    {
        // Stamp EVERY delivered event before any early-out: the counter is the liveness evidence, so it must
        // not depend on the event being one we act on.
        Interlocked.Increment(ref _callbacks);

        if (nCode < 0 || (int)wParam != Native.WM_MOUSEMOVE || !_core.NeedsProcessing)
            return Native.CallNextHookEx(_hook, nCode, wParam, lParam);

        var data = Marshal.PtrToStructure<Native.MSLLHOOKSTRUCT>(lParam);

        // Ignore injected events (includes our own SetCursorPos clamps) — avoids recursion.
        if ((data.flags & (Native.LLMHF_INJECTED | Native.LLMHF_LOWER_IL_INJECTED)) != 0)
            return Native.CallNextHookEx(_hook, nCode, wParam, lParam);

        if (_core.Decide(data.pt.X, data.pt.Y, GateOpen, SideContain, out int bx, out int by) == GuardAction.Block)
        {
            Native.SetCursorPos(bx, by);   // re-enters as injected, which we ignore
            return (IntPtr)1;              // swallow the original move
        }

        return Native.CallNextHookEx(_hook, nCode, wParam, lParam);
    }

    public void Dispose() => Stop();
}
