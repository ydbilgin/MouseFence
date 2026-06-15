using System.Runtime.InteropServices;

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

    /// <summary>True while the hook is installed (the tool is running, not paused).</summary>
    public bool Enabled => _hook != IntPtr.Zero;

    /// <summary>Master toggle: when true the configured crossing gates are active; when false all crossing is blocked.</summary>
    public bool GateOpen { get; set; }

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

    public MouseGuard() => _proc = HookCallback;

    /// <summary>Configure the barrier from the top monitors, the allowed crossing gates, and all monitor rects (for game mode).</summary>
    public void Configure(IEnumerable<Native.RECT> blocked, IEnumerable<(int Min, int Max)> gates, IEnumerable<Native.RECT> allMonitors)
    {
        var rects = blocked.ToList();
        _core.HasTop = rects.Count > 0;
        _core.BarrierY = _core.HasTop ? rects.Max(r => r.Bottom) : 0;
        _core.Gates = gates.Where(g => g.Max > g.Min).ToList();
        _core.Monitors = allMonitors.Select(r => (r.Left, r.Top, r.Right, r.Bottom)).ToList();
    }

    public void Start()
    {
        if (_hook != IntPtr.Zero) return;
        _core.Reset();
        _hook = Native.SetWindowsHookEx(Native.WH_MOUSE_LL, _proc, Native.GetModuleHandle(null), 0);
    }

    public void Stop()
    {
        if (_hook == IntPtr.Zero) return;
        Native.UnhookWindowsHookEx(_hook);
        _hook = IntPtr.Zero;
    }

    private IntPtr HookCallback(int nCode, IntPtr wParam, IntPtr lParam)
    {
        if (nCode < 0 || (int)wParam != Native.WM_MOUSEMOVE || (!_core.HasTop && !_core.Confine))
            return Native.CallNextHookEx(_hook, nCode, wParam, lParam);

        var data = Marshal.PtrToStructure<Native.MSLLHOOKSTRUCT>(lParam);

        // Ignore injected events (includes our own SetCursorPos clamps) — avoids recursion.
        if ((data.flags & (Native.LLMHF_INJECTED | Native.LLMHF_LOWER_IL_INJECTED)) != 0)
            return Native.CallNextHookEx(_hook, nCode, wParam, lParam);

        if (_core.Decide(data.pt.X, data.pt.Y, GateOpen, out int bx, out int by) == GuardAction.Block)
        {
            Native.SetCursorPos(bx, by);   // re-enters as injected, which we ignore
            return (IntPtr)1;              // swallow the original move
        }

        return Native.CallNextHookEx(_hook, nCode, wParam, lParam);
    }

    public void Dispose() => Stop();
}
