use gsy_analytics_engine::period::{align_to_slot, Window};

const SLOT_LENGTH: i64 = 900;
const SLOT: i64 = 1_758_621_600; // 2025-09-23 10:00 UTC

#[test]
fn timestamps_floor_to_the_slot_start() {
    assert_eq!(align_to_slot(SLOT, SLOT_LENGTH), SLOT);
    assert_eq!(align_to_slot(SLOT + 1, SLOT_LENGTH), SLOT);
    assert_eq!(align_to_slot(SLOT + 899, SLOT_LENGTH), SLOT);
    assert_eq!(align_to_slot(SLOT + 900, SLOT_LENGTH), SLOT + 900);
    assert_eq!(align_to_slot(-1, SLOT_LENGTH), -900);
}

#[test]
fn tick_window_ends_at_the_last_settled_slot() {
    // 10:20 with a 15 min delay: only slots ending by 10:05 count, so the window ends at 10:00.
    let window = Window::for_tick(SLOT + 20 * 60, 48, 15, SLOT_LENGTH);
    assert_eq!(window.end, SLOT);
    assert_eq!(window.start, SLOT - 48 * 3600);
    assert_eq!(window.slots().count(), 192);
}

#[test]
fn tick_window_on_an_exact_boundary_includes_the_just_settled_slot() {
    // 10:30 with a 15 min delay: the 10:00-10:15 slot ended exactly 15 min ago.
    let window = Window::for_tick(SLOT + 30 * 60, 1, 15, SLOT_LENGTH);
    assert_eq!(window.end, SLOT + SLOT_LENGTH);
    assert!(window.contains_slot(SLOT));
    assert!(!window.contains_slot(SLOT + SLOT_LENGTH));
}

#[test]
fn window_is_half_open() {
    let window = Window {
        start: SLOT,
        end: SLOT + 2 * SLOT_LENGTH,
        slot_length: SLOT_LENGTH,
    };

    assert!(!window.contains_slot(SLOT - SLOT_LENGTH));
    assert!(window.contains_slot(SLOT));
    assert!(window.contains_slot(SLOT + SLOT_LENGTH));
    assert!(!window.contains_slot(SLOT + 2 * SLOT_LENGTH));
    assert_eq!(
        window.slots().collect::<Vec<_>>(),
        vec![SLOT, SLOT + SLOT_LENGTH]
    );
}
