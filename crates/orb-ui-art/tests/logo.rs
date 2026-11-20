//! task-116: the wordmark is its own part, drawn once over the transport background, which
//! holds no logo of its own. This fork: the ORB-338X logo of the user's top bar (its letters,
//! its subtitle and its icon), each piece on the whole logo canvas for the startup animation.

use orb_ui_art::layout::layout;
use orb_ui_art::parts::{LOGO_H, LOGO_PIECES, LOGO_W, LOGO_X, LOGO_Y, all_parts};

#[test]
fn the_logo_is_its_own_part() {
    let parts = all_parts();
    let get = |name: &str| parts.iter().find(|p| p.name == name).expect(name);
    let logo = get("logo");
    assert_eq!((logo.w, logo.h), (LOGO_W, LOGO_H));
    // the whole is the pieces, in order, each on the same canvas
    for name in LOGO_PIECES {
        let p = get(name);
        assert_eq!((p.w, p.h), (LOGO_W, LOGO_H), "{name}");
        assert!(!p.body.is_empty(), "{name}");
    }
    let inner = |b: &str| {
        let start = b.find('>').unwrap() + 1;
        b[start..b.len() - "</g>".len()].to_string()
    };
    let joined: String = LOGO_PIECES.iter().map(|n| inner(&get(n).body)).collect();
    assert_eq!(inner(&logo.body), joined);
    // the 3, 3 and 8 in red, orange and yellow, the icon's tile clipped
    for (piece, color) in [
        ("logo_3a", "#e2413b"),
        ("logo_3b", "#f08a24"),
        ("logo_8", "#f2c531"),
    ] {
        assert!(get(piece).body.contains(color), "{piece} is {color}");
    }
    assert!(get("logo_icon").body.contains("clip-path"));
    // the transport background has none of it
    assert!(
        !get("bg_transport").body.contains("#e2413b"),
        "bg_transport holds no wordmark"
    );
    // the logo is drawn after the transport background, at the click area of the panel
    let l = layout(&parts);
    let pos = |n: &str| l.backgrounds.iter().position(|b| b.0 == n).expect(n);
    assert!(pos("bg_transport") < pos("logo"));
    let b = &l.backgrounds[pos("logo")];
    assert_eq!((b.1, b.2), (LOGO_X, LOGO_Y));
    // inside the transport section (16..608, 0..50)
    const { assert!(LOGO_X >= 16.0 && LOGO_X + LOGO_W <= 608.0) };
    const { assert!(LOGO_Y >= 0.0 && LOGO_Y + LOGO_H <= 50.0) };
}
