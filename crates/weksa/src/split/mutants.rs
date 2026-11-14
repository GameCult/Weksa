//! Pins of private items of `split` found unpinned by mutation testing.

use super::Checkpoint;

/// A checkpoint reports the round it was declared at, whatever the round.
#[test]
fn a_checkpoint_reports_its_round() {
    assert_eq!(Checkpoint::for_test(7).round(), 7);
    assert_eq!(Checkpoint::for_test(0).round(), 0);
    assert_eq!(Checkpoint::for_test(1).round(), 1);
}
