namespace MouseFence;

/// <summary>One event only. No displacement history, pressure credit, or speed window.</summary>
public static class InstantaneousPush
{
    public static bool Crosses(double primary, double orthogonal, double seconds,
                               double threshold, double slack) =>
        double.IsFinite(primary) && double.IsFinite(orthogonal) && primary > 0 && orthogonal >= 0
        && double.IsFinite(seconds) && seconds > 0
        && double.IsFinite(threshold) && threshold > 0
        && primary / seconds >= threshold && orthogonal <= primary + slack;
}

/// <summary>
/// Consecutive non-injected hook events. DWORD message time is too coarse for sub-ms intervals;
/// a consistent monotonic callback timestamp refines those intervals. Callback time is an estimate,
/// not a hardware timestamp. Queued bursts and pixel quantization cannot be reconstructed here.
/// </summary>
public sealed class HookEventClock
{
    private uint _lastMessage;
    private long _lastCounter;
    private bool _haveLast;

    public void Reset() => _haveLast = false;

    public double Interval(uint messageMs, long counter, long frequency)
    {
        bool haveLast = _haveLast;
        uint elapsed = unchecked(messageMs - _lastMessage);
        long ticks = counter - _lastCounter;
        _lastMessage = messageMs; _lastCounter = counter; _haveLast = true;
        if (!haveLast || frequency <= 0 || ticks <= 0 || elapsed > 100) return double.NaN;
        double callbackMs = 1000.0 * ticks / frequency;
        // Reject backwards time, idle gaps and obvious dispatch compression/stalls. 0.0625 ms is
        // half an 8000 Hz interval; a burst faster than that carries no trustworthy event duration.
        if (callbackMs < 0.0625 || callbackMs > 100 || Math.Abs(callbackMs - elapsed) > 1.0)
            return double.NaN;
        // A longer message interval can describe one coalesced move: divide that event's whole
        // displacement by its whole duration. Never add displacement from preceding events.
        return elapsed > 1 ? elapsed / 1000.0 : callbackMs / 1000.0;
    }
}

/// <summary>Opt-in, current-session learning from individual blocked arrivals, separately by axis.</summary>
internal sealed class ArrivalLearner
{
    private readonly double[] _samples = new double[64];
    private int _count, _head;
    public double Threshold { get; private set; }
    public void Reset() { _count = _head = 0; Threshold = 0; }
    public void Observe(double speed)
    {
        if (!double.IsFinite(speed) || speed <= 0) return;
        _samples[_head] = speed; _head = (_head + 1) % _samples.Length;
        _count = Math.Min(_samples.Length, _count + 1);
        if (_count < 40) return;
        Span<double> ordered = stackalloc double[_count];
        _samples.AsSpan(0, _count).CopyTo(ordered);
        ordered.Sort();
        Threshold = Math.Clamp(ordered[(int)Math.Ceiling(_count * 0.95) - 1] * 1.2, 1500, 12000);
    }
}
