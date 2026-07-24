namespace MouseFence;

/// <summary>Result of one liveness probe of the low-level mouse hook.</summary>
public enum HookHealthVerdict
{
    /// <summary>Hook traffic arrived since the previous probe — the hook is installed and delivering.</summary>
    Alive,

    /// <summary>Nothing moved since the previous probe, so the probe proves nothing either way.</summary>
    Inconclusive,

    /// <summary>The cursor moved yet no hook traffic arrived — Windows silently unhooked us.</summary>
    Dead,
}

/// <summary>
/// Decides, from observable evidence only, whether the low-level mouse hook is still delivering.
///
/// Windows silently removes a WH_MOUSE_LL hook whose callback overruns LowLevelHooksTimeout, and it does
/// NOT clear the handle the process holds — so a non-zero hook handle is not evidence that the fence is up.
/// The only real evidence is traffic: if the cursor moved between two probes and not a single callback
/// arrived in that window, the hook is gone. No movement means no evidence, and never a verdict — the
/// asymmetry matters, because "Dead" triggers a reinstall and a false positive would tear down a live hook.
/// </summary>
public static class HookHealth
{
    public static HookHealthVerdict Evaluate(bool cursorMoved, long callbacksSincePreviousProbe)
    {
        if (callbacksSincePreviousProbe > 0)
            return HookHealthVerdict.Alive;

        return cursorMoved ? HookHealthVerdict.Dead : HookHealthVerdict.Inconclusive;
    }
}
