using System.Runtime.InteropServices;

namespace MouseFence;

/// <summary>
/// Installs the low-level mouse hook and applies <see cref="GuardCore"/>'s decision: a one-way barrier
/// above the bottom row of monitors. See <see cref="GuardCore"/> for the (tested) decision logic.
/// </summary>
public sealed class MouseGuard : IDisposable
{
    private const int GateInset = 24;   // shrink the main-screen gate by this many px on each side

    private IntPtr _hook = IntPtr.Zero;
    private readonly Native.HookProc _proc;   // kept alive so the GC never collects the callback
    private readonly GuardCore _core = new();

    /// <summary>True while the hook is installed (the tool is running, not paused).</summary>
    public bool Enabled => _hook != IntPtr.Zero;

    /// <summary>When true, a deliberate push up from the MAIN screen is honoured. Side screens are never affected.</summary>
    public bool GateOpen { get; set; }

    public MouseGuard() => _proc = HookCallback;

    public void Configure(IEnumerable<Native.RECT> blocked, int gateLeft, int gateRight)
    {
        var rects = blocked.ToList();
        _core.HasTop = rects.Count > 0;
        _core.BarrierY = _core.HasTop ? rects.Max(r => r.Bottom) : 0;

        int inset = Math.Min(GateInset, Math.Max(0, (gateRight - gateLeft) / 4));
        _core.GateLeft = gateLeft + inset;
        _core.GateRight = gateRight - inset;
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
        if (nCode < 0 || (int)wParam != Native.WM_MOUSEMOVE || !_core.HasTop)
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
