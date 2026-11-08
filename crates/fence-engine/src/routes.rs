//! Descent-route derivation (port of C# `GuardCore.DeriveRoutes`, Core/GuardCore.Routes.cs). Track A.
//!
//! Order matters: the first matching route wins in `decide`, and C# groups links by target in first-seen order.
//! Every id comparison here is exact (C# default comparers: `GroupBy`, `Distinct`, `HashSet<string>`,
//! `Dictionary<string, _>`).

use fence_types::Rect;

/// C# `DeriveRoutes(links, rects, topDevices)`. `links` are (from, to) stable-id pairs; `rects` maps a stable
/// id to its bounds (a later duplicate key wins, like repeated dictionary assignment); `top_devices` are stable
/// ids.
///
/// Three guards: a top is routed only when it has exactly one distinct from-device (ambiguity); the landing must
/// sit genuinely below the top (`landing.t >= top.b`, SF-1); top and landing must share a positive X span.
pub fn derive_routes(
    links: &[(String, String)],
    rects: &[(String, Rect)],
    top_devices: &[String],
) -> Vec<(Rect, Rect)> {
    let lookup = |id: &str| rects.iter().rev().find(|(k, _)| k == id).map(|(_, r)| *r);
    let mut routes = Vec::new();
    // GroupBy(lk => lk.To) over the links whose target is a top device, groups in first-seen key order.
    let mut keys: Vec<&str> = Vec::new();
    for (_, to) in links {
        if top_devices.iter().any(|t| t == to) && !keys.contains(&to.as_str()) {
            keys.push(to);
        }
    }
    for key in keys {
        let mut froms: Vec<&str> = Vec::new();
        for (from, to) in links {
            if to == key && !froms.contains(&from.as_str()) {
                froms.push(from);
            }
        }
        if froms.len() != 1 {
            continue; // ambiguity guard
        }
        let Some(landing) = lookup(froms[0]) else { continue };
        let Some(top) = lookup(key) else { continue };
        if landing.t < top.b {
            continue; // SF-1: Landing.T >= Top.B
        }
        if top.r.min(landing.r) <= top.l.max(landing.l) {
            continue; // positive X overlap
        }
        routes.push((top, landing));
    }
    routes
}
