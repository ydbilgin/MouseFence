namespace MouseFence;

/// <summary>A hidden message window that receives WM_HOTKEY for a single global hotkey.</summary>
public sealed class HotKeyWindow : NativeWindow, IDisposable
{
    private const int HotKeyId = 0xB10C;

    public event Action HotKeyPressed;

    public HotKeyWindow() => CreateHandle(new CreateParams());

    public bool Register(uint modifiers, uint vk)
    {
        Native.UnregisterHotKey(Handle, HotKeyId);
        if (modifiers == 0 || vk == 0) return false;
        return Native.RegisterHotKey(Handle, HotKeyId, modifiers | Native.MOD_NOREPEAT, vk);
    }

    public void Unregister() => Native.UnregisterHotKey(Handle, HotKeyId);

    protected override void WndProc(ref Message m)
    {
        if (m.Msg == Native.WM_HOTKEY && (int)m.WParam == HotKeyId)
            HotKeyPressed?.Invoke();
        base.WndProc(ref m);
    }

    public void Dispose()
    {
        Unregister();
        if (Handle != IntPtr.Zero)
            DestroyHandle();
    }
}
