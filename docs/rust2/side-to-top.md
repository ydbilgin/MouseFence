# Automatic side-to-top precedence

Automatic upward boundaries from a non-primary side display to a configured top
display are Blocked. In a normal T layout, route through MAIN in separate moves;
a single report through MAIN cannot bypass the side display's upper boundary.

The existing anti-trap safety takes precedence when the top is the side display's
only exit: with safety escapes enabled, this exit is Push even with the Up gate
closed and even without compile-time geometry repair. A firm, deliberate push
crosses; incidental edge contact, a one-pixel nudge or resting does not. All three
crossing models use their normal intent judge. This exception is never Free.

Disabling safety escapes is the explicit expert override and can seal the side
display. Explicit rules and geometry-safety validation otherwise keep their
existing precedence. `side_top.rs` covers the sole-exit and normal T layouts,
geometry safety on/off, live gate open/closed and all three models.
