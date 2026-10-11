use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::super::reorder::Reorder;

#[test]
fn items_come_out_in_index_order_whatever_order_they_land() {
    let r = Reorder::new(4, 4);
    let claimed: Vec<usize> = (0..4).map(|_| r.claim().unwrap()).collect();
    assert_eq!(claimed, [0, 1, 2, 3]);
    assert_eq!(r.claim(), None);
    for i in [2, 0, 3, 1] {
        r.put(i, i * 10);
    }
    let taken: Vec<usize> = std::iter::from_fn(|| r.take()).collect();
    assert_eq!(taken, [0, 10, 20, 30]);
}

#[test]
fn a_claim_waits_until_the_window_has_room() {
    let r = Reorder::new(10, 2);
    assert_eq!(r.claim(), Some(0));
    assert_eq!(r.claim(), Some(1));
    std::thread::scope(|s| {
        let third = s.spawn(|| r.claim());
        std::thread::sleep(Duration::from_millis(50));
        assert!(!third.is_finished(), "index 2 is outside the window");
        r.put(0, 'a');
        assert_eq!(r.take(), Some('a'));
        assert_eq!(third.join().unwrap(), Some(2));
    });
}

#[test]
fn stop_wakes_a_waiting_reader_and_claimer() {
    let r: Reorder<u8> = Reorder::new(5, 1);
    assert_eq!(r.claim(), Some(0));
    std::thread::scope(|s| {
        let reader = s.spawn(|| r.take());
        let claimer = s.spawn(|| r.claim());
        std::thread::sleep(Duration::from_millis(30));
        r.stop();
        assert_eq!(reader.join().unwrap(), None);
        assert_eq!(claimer.join().unwrap(), None);
    });
}

#[test]
fn many_workers_never_hold_more_than_the_window() {
    let total = 400;
    let window = 6;
    let r = Reorder::new(total, window);
    let most = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for w in 0..3 {
            let (r, most) = (&r, &most);
            s.spawn(move || {
                while let Some(i) = r.claim() {
                    // Uneven work so items land out of order.
                    if (i + w) % 5 == 0 {
                        std::thread::sleep(Duration::from_micros(300));
                    }
                    r.put(i, i);
                    most.fetch_max(r.held(), Ordering::Relaxed);
                }
            });
        }
        for expected in 0..total {
            assert_eq!(r.take(), Some(expected));
        }
        assert_eq!(r.take(), None);
    });
    assert!(most.load(Ordering::Relaxed) <= window);
}
