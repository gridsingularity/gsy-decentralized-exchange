use gsy_analytics_engine::db::next_backoff;
use std::time::Duration;

#[test]
fn backoff_doubles_and_caps_at_thirty_seconds() {
    let mut delay = Duration::from_secs(1);
    let mut delays = vec![delay];
    for _ in 0..6 {
        delay = next_backoff(delay);
        delays.push(delay);
    }

    let seconds: Vec<u64> = delays.iter().map(Duration::as_secs).collect();
    assert_eq!(seconds, vec![1, 2, 4, 8, 16, 30, 30]);
}
