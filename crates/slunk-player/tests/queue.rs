//! The queue's rules (backend.md, "The queue"): Previous, Next, repeat,
//! shuffle, Play next and Add to queue, moving and removing entries,
//! failures, what plays next, and what's remembered.

use slunk_player::queue::{self, Entry, Queue, Repeat, Step};

/// The queue after these steps, from an empty one.
fn after(steps: impl IntoIterator<Item = Step>) -> Queue {
    let mut queue = Queue::default();
    for step in steps {
        queue.step(step);
    }
    queue
}

/// The queue after one more step.
fn stepped(queue: &Queue, step: Step) -> Queue {
    let mut queue = queue.clone();
    queue.step(step);
    queue
}

fn play(track_ids: &[i64], start: Option<i64>, seed: u32) -> Step {
    Step::Play {
        track_ids: track_ids.to_vec(),
        start,
        seed,
    }
}

/// A queue of tracks 10, 20 and 30, playing from `start`.
fn playing(start: i64) -> Step {
    play(&[10, 20, 30], Some(start), 1)
}

/// The current entry's track.
fn track(queue: &Queue) -> Option<i64> {
    queue.current_entry().map(|entry| entry.track_id)
}

/// The queue's tracks, in the order they play.
fn tracks(queue: &Queue) -> Vec<i64> {
    queue.entries.iter().map(|entry| entry.track_id).collect()
}

/// The track up next.
fn next(queue: &Queue) -> Option<i64> {
    queue.up_next().map(|entry| entry.track_id)
}

/// A queue of tracks 10 to 60, with shuffle and repeat set, from `start`.
fn modes(shuffle: bool, repeat: Repeat, start: Option<i64>) -> Queue {
    after([
        Step::SetShuffle { shuffle, seed: 7 },
        Step::SetRepeat(repeat),
        play(&[10, 20, 30, 40, 50, 60], start, 7),
    ])
}

mod starting_a_queue {
    use super::*;

    #[test]
    fn plays_the_tracks_from_the_one_asked_for() {
        let queue = after([playing(1)]);
        assert_eq!(tracks(&queue), [10, 20, 30]);
        assert_eq!(track(&queue), Some(20));
        assert!(queue.playing);
    }

    #[test]
    fn gives_a_track_queued_twice_two_entries() {
        let queue = after([play(&[10, 10], Some(0), 1)]);
        let [first, second] = queue.entries[..] else {
            panic!("two entries")
        };
        assert_eq!(first.track_id, second.track_id);
        assert_ne!(first.key, second.key);
    }

    #[test]
    fn replaces_the_queue_with_new_keys() {
        let first = after([playing(0)]);
        let second = stepped(&first, playing(0));
        assert!(
            second
                .entries
                .iter()
                .all(|entry| first.entries.iter().all(|old| old.key != entry.key))
        );
    }

    #[test]
    fn keeps_the_start_within_the_queue() {
        assert_eq!(track(&after([playing(7)])), Some(30));
        assert_eq!(track(&after([playing(-1)])), Some(10));
    }

    #[test]
    fn empties_the_queue_with_no_tracks() {
        let queue = after([playing(0), play(&[], Some(0), 1)]);
        assert_eq!(queue.current_entry(), None);
        assert!(!queue.playing);
    }
}

#[test]
fn with_nothing_queued_does_nothing() {
    for step in [
        Step::Toggle,
        Step::SetPlaying(true),
        Step::Next,
        Step::Previous { time: 0.0 },
        Step::Ended,
        Step::Failed,
        Step::Handoff,
    ] {
        assert_eq!(stepped(&Queue::default(), step), Queue::default());
    }
}

mod play_and_pause {
    use super::*;

    #[test]
    fn toggles() {
        let paused = after([playing(0), Step::Toggle]);
        assert!(!paused.playing);
        assert!(stepped(&paused, Step::Toggle).playing);
    }

    #[test]
    fn follows_what_it_is_told() {
        let queue = after([playing(0)]);
        assert_eq!(stepped(&queue, Step::SetPlaying(true)), queue);
        assert!(!stepped(&queue, Step::SetPlaying(false)).playing);
    }
}

mod next {
    use super::*;

    #[test]
    fn goes_to_the_next_track_playing_or_paused_as_it_was() {
        let queue = after([playing(0), Step::Next]);
        assert_eq!((queue.current, queue.playing), (Some(1), true));
        let queue = after([playing(0), Step::Toggle, Step::Next]);
        assert_eq!((queue.current, queue.playing), (Some(1), false));
    }

    #[test]
    fn stops_after_the_last_track_at_its_start() {
        let last = after([playing(2)]);
        let stopped = stepped(&last, Step::Next);
        assert_eq!(track(&stopped), Some(30));
        assert!(!stopped.playing);
        assert_eq!(stopped.restarts, last.restarts + 1);
    }
}

mod previous {
    use super::*;

    #[test]
    fn goes_back_to_the_previous_track_in_the_first_seconds() {
        let queue = after([playing(2), Step::Previous { time: 3.0 }]);
        assert_eq!(track(&queue), Some(20));
        assert!(queue.playing);
    }

    #[test]
    fn goes_back_to_the_tracks_start_after_them() {
        let queue = after([playing(2)]);
        let restarted = stepped(&queue, Step::Previous { time: 3.5 });
        assert_eq!(track(&restarted), Some(30));
        assert_eq!(restarted.restarts, queue.restarts + 1);
    }

    #[test]
    fn goes_back_to_the_first_tracks_start() {
        let queue = after([playing(0)]);
        let restarted = stepped(&queue, Step::Previous { time: 0.0 });
        assert_eq!(track(&restarted), Some(10));
        assert_eq!(restarted.restarts, queue.restarts + 1);
    }

    #[test]
    fn stays_paused_when_paused() {
        let queue = after([playing(1), Step::Toggle, Step::Previous { time: 0.0 }]);
        assert_eq!((queue.current, queue.playing), (Some(0), false));
    }
}

mod a_track_that_ends {
    use super::*;

    #[test]
    fn goes_on_to_the_next_one_playing() {
        let queue = after([playing(0), Step::SetPlaying(false), Step::Ended]);
        assert_eq!((queue.current, queue.playing), (Some(1), true));
    }

    #[test]
    fn stops_the_queue_when_its_the_last() {
        let queue = after([playing(2), Step::Ended]);
        assert_eq!((queue.current, queue.playing), (Some(2), false));
    }
}

mod a_track_that_cant_play {
    use super::*;

    #[test]
    fn is_skipped() {
        let queue = after([playing(0), Step::Failed]);
        assert_eq!(
            (queue.current, queue.playing, queue.failures),
            (Some(1), true, 1)
        );
    }

    #[test]
    fn stops_the_queue_once_every_track_has_failed_in_a_row() {
        let queue = after([playing(0), Step::Failed, Step::Failed, Step::Failed]);
        assert!(!queue.playing);
        assert_eq!(queue.failures, 3);
    }

    #[test]
    fn counts_again_once_a_track_plays() {
        let queue = after([playing(0), Step::Failed, Step::Started, Step::Failed]);
        assert_eq!(
            (queue.current, queue.playing, queue.failures),
            (Some(2), true, 1)
        );
    }
}

mod jumping_to_an_entry {
    use super::*;

    #[test]
    fn plays_it() {
        let queue = after([playing(0), Step::Toggle]);
        let third = queue.entries[2];
        let jumped = stepped(&queue, Step::Jump { key: third.key });
        assert_eq!((jumped.current, jumped.playing), (Some(2), true));
    }

    #[test]
    fn plays_the_current_one_from_its_start() {
        let queue = after([playing(1)]);
        let jumped = stepped(
            &queue,
            Step::Jump {
                key: queue.entries[1].key,
            },
        );
        assert_eq!(jumped.current, Some(1));
        assert_eq!(jumped.restarts, queue.restarts + 1);
    }

    #[test]
    fn ignores_an_entry_that_isnt_queued() {
        let queue = after([playing(0)]);
        assert_eq!(stepped(&queue, Step::Jump { key: 999 }), queue);
    }
}

mod removing_entries {
    use super::*;

    #[test]
    fn keeps_the_current_one_current() {
        let queue = after([playing(1)]);
        let removed = stepped(
            &queue,
            Step::Remove {
                key: queue.entries[0].key,
            },
        );
        assert_eq!(track(&removed), Some(20));
        assert_eq!(removed.current, Some(0));
    }

    #[test]
    fn goes_on_to_the_next_one_when_the_current_one_goes() {
        let queue = after([playing(1), Step::Toggle]);
        let removed = stepped(
            &queue,
            Step::Remove {
                key: queue.entries[1].key,
            },
        );
        assert_eq!(track(&removed), Some(30));
        assert!(!removed.playing);
    }

    #[test]
    fn stops_at_the_last_one_left_when_the_last_one_goes() {
        let queue = after([playing(2)]);
        let removed = stepped(
            &queue,
            Step::Remove {
                key: queue.entries[2].key,
            },
        );
        assert_eq!(track(&removed), Some(20));
        assert!(!removed.playing);
    }

    #[test]
    fn removes_one_entry_of_a_track_queued_twice() {
        let queue = after([play(&[10, 20, 10], Some(0), 1)]);
        let removed = stepped(
            &queue,
            Step::Remove {
                key: queue.entries[2].key,
            },
        );
        assert_eq!(tracks(&removed), [10, 20]);
    }

    #[test]
    fn empties_the_queue_when_the_only_one_goes() {
        let queue = after([play(&[10], Some(0), 1)]);
        let removed = stepped(
            &queue,
            Step::Remove {
                key: queue.entries[0].key,
            },
        );
        assert_eq!(removed.current_entry(), None);
        assert!(!removed.playing);
    }
}

mod deleted_tracks {
    use super::*;

    #[test]
    fn leave_the_queue_every_entry_of_them() {
        let queue = after([play(&[10, 20, 10, 30], Some(3), 1)]);
        let kept = stepped(
            &queue,
            Step::Keep {
                track_ids: vec![20, 30],
            },
        );
        assert_eq!(tracks(&kept), [20, 30]);
        assert_eq!(track(&kept), Some(30));
    }

    #[test]
    fn move_the_queue_on_when_the_current_one_is_deleted() {
        let queue = after([playing(1)]);
        let kept = stepped(
            &queue,
            Step::Keep {
                track_ids: vec![10, 30],
            },
        );
        assert_eq!((kept.current, kept.playing), (Some(1), true));
        assert_eq!(track(&kept), Some(30));
    }

    #[test]
    fn change_nothing_when_every_track_is_still_there() {
        let queue = after([playing(0)]);
        assert_eq!(
            stepped(
                &queue,
                Step::Keep {
                    track_ids: vec![10, 20, 30, 40]
                }
            ),
            queue
        );
    }
}

mod clearing {
    use super::*;

    #[test]
    fn empties_the_queue_and_stops() {
        let queue = after([playing(0), Step::Clear]);
        assert!(queue.entries.is_empty());
        assert_eq!(queue.current_entry(), None);
        assert!(!queue.playing);
    }

    #[test]
    fn never_hands_out_a_key_again() {
        let first = after([playing(0)]);
        let second = after([playing(0), Step::Clear, playing(0)]);
        assert!(second.entries[0].key > first.entries[2].key);
    }
}

mod play_next_and_add_to_queue {
    use super::*;

    #[test]
    fn put_tracks_right_after_the_current_one_or_at_the_end() {
        let queue = after([playing(1)]);
        let next = stepped(
            &queue,
            Step::PlayNext {
                track_ids: vec![1, 2],
            },
        );
        assert_eq!(tracks(&next), [10, 20, 1, 2, 30]);
        let added = stepped(
            &queue,
            Step::Add {
                track_ids: vec![1, 2],
            },
        );
        assert_eq!(tracks(&added), [10, 20, 30, 1, 2]);
    }

    #[test]
    fn leave_the_current_track_playing() {
        let queue = after([playing(1), Step::PlayNext { track_ids: vec![1] }]);
        assert_eq!((queue.current, queue.playing), (Some(1), true));
    }

    #[test]
    fn start_a_queue_paused_at_its_first_track_when_nothings_queued() {
        for step in [
            Step::PlayNext {
                track_ids: vec![1, 2],
            },
            Step::Add {
                track_ids: vec![1, 2],
            },
        ] {
            let queue = after([step]);
            assert_eq!(tracks(&queue), [1, 2]);
            assert_eq!((queue.current, queue.playing), (Some(0), false));
        }
    }

    #[test]
    fn give_each_added_track_a_new_key() {
        let queue = after([
            playing(0),
            Step::Add {
                track_ids: vec![10],
            },
        ]);
        let mut keys: Vec<u64> = queue.entries.iter().map(|entry| entry.key).collect();
        keys.dedup();
        assert_eq!(keys.len(), 4);
    }
}

mod moving_an_entry {
    use super::*;

    #[test]
    fn moves_it_in_the_order_they_play_keeping_the_current_one_current() {
        let queue = after([playing(1)]);
        let moved = stepped(
            &queue,
            Step::Move {
                key: queue.entries[2].key,
                by: -2,
            },
        );
        assert_eq!(tracks(&moved), [30, 10, 20]);
        assert_eq!(track(&moved), Some(20));
    }

    #[test]
    fn moves_the_current_one_with_it() {
        let queue = after([playing(1)]);
        let moved = stepped(
            &queue,
            Step::Move {
                key: queue.entries[1].key,
                by: 1,
            },
        );
        assert_eq!(tracks(&moved), [10, 30, 20]);
        assert_eq!(moved.current, Some(2));
    }

    #[test]
    fn moves_it_several_places_at_once_past_the_current_one() {
        let queue = after([play(&[10, 20, 30, 40, 50], Some(2), 1)]);
        let down = stepped(
            &queue,
            Step::Move {
                key: queue.entries[0].key,
                by: 3,
            },
        );
        assert_eq!(tracks(&down), [20, 30, 40, 10, 50]);
        assert_eq!(track(&down), Some(30));
        let up = stepped(
            &queue,
            Step::Move {
                key: queue.entries[4].key,
                by: -4,
            },
        );
        assert_eq!(tracks(&up), [50, 10, 20, 30, 40]);
        assert_eq!(track(&up), Some(30));
    }

    #[test]
    fn cant_move_past_either_end() {
        let queue = after([playing(0)]);
        let first = Step::Move {
            key: queue.entries[0].key,
            by: -1,
        };
        assert_eq!(stepped(&queue, first), queue);
    }
}

mod shuffle {
    use super::*;

    #[test]
    fn plays_the_entries_after_the_current_one_in_another_order() {
        let queue = after([
            play(&[10, 20, 30, 40, 50, 60], Some(1), 7),
            Step::SetShuffle {
                shuffle: true,
                seed: 7,
            },
        ]);
        assert_eq!(tracks(&queue)[..2], [10, 20]);
        let mut rest = tracks(&queue)[2..].to_vec();
        assert_ne!(rest, [30, 40, 50, 60]);
        rest.sort();
        assert_eq!(rest, [30, 40, 50, 60]);
        assert_eq!(track(&queue), Some(20));
    }

    #[test]
    fn is_the_same_for_the_same_seed() {
        assert_eq!(
            tracks(&modes(true, Repeat::Off, None)),
            tracks(&modes(true, Repeat::Off, None))
        );
    }

    #[test]
    fn off_plays_the_rest_in_their_order_again_from_the_current_one() {
        let shuffled = modes(true, Repeat::Off, None);
        let current = track(&shuffled);
        let off = stepped(
            &shuffled,
            Step::SetShuffle {
                shuffle: false,
                seed: 1,
            },
        );
        assert_eq!(track(&off), current);
        let rest = tracks(&off)[off.current.unwrap() + 1..].to_vec();
        let mut sorted = rest.clone();
        sorted.sort();
        assert_eq!(rest, sorted);
        assert_eq!(off.unshuffled, None);
    }

    #[test]
    fn starts_a_new_queue_at_the_track_that_was_clicked() {
        let queue = modes(true, Repeat::Off, Some(3));
        assert_eq!(track(&queue), Some(40));
        assert_eq!(queue.current, Some(0));
        let mut all = tracks(&queue);
        all.sort();
        assert_eq!(all, [10, 20, 30, 40, 50, 60]);
    }

    #[test]
    fn starts_a_new_queue_at_a_random_track_without_one() {
        let mut firsts: Vec<Option<i64>> = (1..=8)
            .map(|seed| {
                track(&after([
                    Step::SetShuffle {
                        shuffle: true,
                        seed,
                    },
                    play(&[10, 20, 30, 40, 50, 60], None, seed),
                ]))
            })
            .collect();
        firsts.sort();
        firsts.dedup();
        assert!(firsts.len() > 1);
    }

    #[test]
    fn keeps_play_next_right_after_the_current_one_and_add_at_the_end() {
        let queue = after([
            play(&[10, 20, 30, 40], Some(0), 7),
            Step::SetShuffle {
                shuffle: true,
                seed: 7,
            },
            Step::PlayNext { track_ids: vec![1] },
            Step::Add { track_ids: vec![2] },
        ]);
        assert_eq!(tracks(&queue)[1], 1);
        assert_eq!(tracks(&queue).last(), Some(&2));
        // Unshuffled, they keep those places in the order before
        let off = stepped(
            &queue,
            Step::SetShuffle {
                shuffle: false,
                seed: 1,
            },
        );
        assert_eq!(tracks(&off), [10, 1, 20, 30, 40, 2]);
    }

    #[test]
    fn forgets_removed_entries_in_the_order_before() {
        let queue = after([
            play(&[10, 20, 30], Some(0), 7),
            Step::SetShuffle {
                shuffle: true,
                seed: 7,
            },
        ]);
        let twenty = queue
            .entries
            .iter()
            .find(|entry| entry.track_id == 20)
            .unwrap();
        let removed = stepped(&queue, Step::Remove { key: twenty.key });
        let before: Vec<i64> = removed
            .unshuffled
            .unwrap()
            .iter()
            .map(|entry| entry.track_id)
            .collect();
        assert_eq!(before, [10, 30]);
    }
}

mod repeat {
    use super::*;

    #[test]
    fn goes_from_off_to_all_to_one_and_back_to_off() {
        assert_eq!(Repeat::Off.next(), Repeat::All);
        assert_eq!(Repeat::All.next(), Repeat::One);
        assert_eq!(Repeat::One.next(), Repeat::Off);
    }

    #[test]
    fn all_the_first_entry_follows_the_last() {
        let last = modes(false, Repeat::All, Some(5));
        let next = stepped(&last, Step::Next);
        assert_eq!((next.current, next.playing), (Some(0), true));
        let ended = stepped(&stepped(&last, Step::SetPlaying(false)), Step::Ended);
        assert_eq!((ended.current, ended.playing), (Some(0), true));
    }

    #[test]
    fn all_previous_on_the_first_entry_goes_to_the_last() {
        let first = modes(false, Repeat::All, Some(0));
        assert_eq!(
            stepped(&first, Step::Previous { time: 0.0 }).current,
            Some(5)
        );
    }

    #[test]
    fn all_a_queue_of_one_track_starts_it_again() {
        let one = after([Step::SetRepeat(Repeat::All), play(&[10], None, 1)]);
        let again = stepped(&one, Step::Ended);
        assert_eq!((again.current, again.playing), (Some(0), true));
        assert_eq!(again.restarts, one.restarts + 1);
    }

    #[test]
    fn one_a_track_that_ends_plays_again() {
        let queue = modes(false, Repeat::One, Some(2));
        let again = stepped(&queue, Step::Ended);
        assert_eq!((again.current, again.playing), (Some(2), true));
        assert_eq!(again.restarts, queue.restarts + 1);
    }

    #[test]
    fn one_next_and_previous_still_move_and_the_queue_still_stops() {
        let queue = modes(false, Repeat::One, Some(2));
        assert_eq!(stepped(&queue, Step::Next).current, Some(3));
        assert_eq!(
            stepped(&queue, Step::Previous { time: 0.0 }).current,
            Some(1)
        );
        let last = stepped(&modes(false, Repeat::One, Some(5)), Step::Next);
        assert_eq!((last.current, last.playing), (Some(5), false));
    }

    #[test]
    fn one_a_track_that_cant_play_is_skipped_not_tried_again() {
        let queue = modes(false, Repeat::One, Some(2));
        assert_eq!(stepped(&queue, Step::Failed).current, Some(3));
    }

    #[test]
    fn all_stops_once_every_entry_has_failed_rather_than_going_round() {
        let mut queue = modes(false, Repeat::All, Some(4));
        for _ in 0..6 {
            queue.step(Step::Failed);
        }
        assert!(!queue.playing);
        assert_eq!(queue.failures, 6);
    }

    #[test]
    fn survive_clearing_the_queue() {
        let queue = stepped(&modes(true, Repeat::One, None), Step::Clear);
        assert!(queue.shuffle);
        assert_eq!(queue.repeat, Repeat::One);
    }
}

mod what_plays_next {
    use super::*;

    #[test]
    fn is_the_next_entry() {
        assert_eq!(next(&after([playing(0)])), Some(20));
        let queue = after([playing(1)]);
        assert_eq!(queue.up_next(), Some(queue.entries[2]));
    }

    #[test]
    fn is_nothing_after_the_last_or_with_nothing_queued() {
        assert_eq!(after([playing(2)]).up_next(), None);
        assert_eq!(Queue::default().up_next(), None);
    }

    #[test]
    fn is_the_first_after_the_last_with_repeat_all() {
        let last = modes(false, Repeat::All, Some(5));
        assert_eq!(last.up_next(), Some(last.entries[0]));
    }

    #[test]
    fn is_the_only_entry_again_with_repeat_all() {
        let one = after([Step::SetRepeat(Repeat::All), play(&[10], None, 1)]);
        assert_eq!(one.up_next(), one.current_entry());
    }

    #[test]
    fn is_the_current_entry_again_with_repeat_one_the_last_one_too() {
        for start in [2, 5] {
            let queue = modes(false, Repeat::One, Some(start));
            assert_eq!(queue.up_next(), queue.current_entry());
        }
    }

    #[test]
    fn follows_the_shuffled_order() {
        let queue = modes(true, Repeat::Off, None);
        assert_eq!(queue.up_next(), Some(queue.entries[1]));
    }

    #[test]
    fn is_what_a_track_that_ends_goes_on_to() {
        for queue in [
            after([playing(0)]),
            modes(false, Repeat::All, Some(5)),
            modes(true, Repeat::Off, Some(2)),
        ] {
            let ended = stepped(&queue, Step::Ended);
            assert_eq!(ended.current_entry(), queue.up_next());
        }
    }

    #[test]
    fn changes_with_the_queue() {
        let queue = after([playing(0)]);
        let ahead = queue.up_next().unwrap();
        // Play next and add to queue
        assert_eq!(
            next(&stepped(&queue, Step::PlayNext { track_ids: vec![1] })),
            Some(1)
        );
        assert_eq!(
            next(&stepped(
                &after([playing(2)]),
                Step::Add { track_ids: vec![1] }
            )),
            Some(1)
        );
        // A move, a removal, and clearing
        let moved = stepped(
            &queue,
            Step::Move {
                key: ahead.key,
                by: 1,
            },
        );
        assert_eq!(next(&moved), Some(30));
        assert_eq!(
            next(&stepped(&queue, Step::Remove { key: ahead.key })),
            Some(30)
        );
        assert_eq!(stepped(&queue, Step::Clear).up_next(), None);
        // Shuffle and repeat
        let shuffled = stepped(
            &modes(false, Repeat::Off, None),
            Step::SetShuffle {
                shuffle: true,
                seed: 7,
            },
        );
        assert_eq!(shuffled.up_next(), Some(shuffled.entries[1]));
        assert_ne!(next(&shuffled), Some(20));
        let one = stepped(&queue, Step::SetRepeat(Repeat::One));
        assert_eq!(one.up_next(), queue.current_entry());
        let all = stepped(&after([playing(2)]), Step::SetRepeat(Repeat::All));
        assert_eq!(next(&all), Some(10));
    }
}

mod what_follows {
    use super::*;

    fn following(queue: &Queue) -> Vec<i64> {
        queue.following().map(|entry| entry.track_id).collect()
    }

    #[test]
    fn is_the_rest_of_the_queue() {
        assert_eq!(following(&after([playing(0)])), [20, 30]);
        assert_eq!(following(&after([playing(2)])), [] as [i64; 0]);
        assert_eq!(following(&Queue::default()), [] as [i64; 0]);
    }

    #[test]
    fn comes_round_with_repeat_all_up_to_the_current_one() {
        assert_eq!(
            following(&modes(false, Repeat::All, Some(4))),
            [60, 10, 20, 30, 40]
        );
    }

    #[test]
    fn is_the_current_one_again_with_repeat_one_or_one_entry_on_repeat_all() {
        assert_eq!(following(&modes(false, Repeat::One, Some(2))), [30]);
        let one = after([Step::SetRepeat(Repeat::All), play(&[10], None, 1)]);
        assert_eq!(following(&one), [10]);
    }

    #[test]
    fn starts_with_what_plays_next() {
        for queue in [
            after([playing(0)]),
            modes(false, Repeat::All, Some(5)),
            modes(false, Repeat::One, Some(3)),
            modes(true, Repeat::Off, Some(2)),
        ] {
            assert_eq!(queue.following().next(), queue.up_next());
        }
    }
}

mod the_handoff {
    use super::*;

    #[test]
    fn goes_on_to_what_was_up_next_playing() {
        let queue = after([playing(0)]);
        let handed_off = stepped(&queue, Step::Handoff);
        assert_eq!(handed_off.current_entry(), queue.up_next());
        assert_eq!((handed_off.current, handed_off.playing), (Some(1), true));
        assert_eq!(handed_off.restarts, queue.restarts);
    }

    #[test]
    fn goes_on_to_the_first_after_the_last_with_repeat_all() {
        let last = modes(false, Repeat::All, Some(5));
        let handed_off = stepped(&last, Step::Handoff);
        assert_eq!((handed_off.current, handed_off.playing), (Some(0), true));
    }

    #[test]
    fn doesnt_restart_the_same_entry_which_already_plays_again() {
        for queue in [
            modes(false, Repeat::One, Some(2)),
            after([Step::SetRepeat(Repeat::All), play(&[10], None, 1)]),
        ] {
            assert_eq!(stepped(&queue, Step::Handoff), queue);
        }
    }

    #[test]
    fn follows_the_shuffled_order() {
        let queue = modes(true, Repeat::Off, Some(2));
        assert_eq!(
            stepped(&queue, Step::Handoff).current_entry(),
            Some(queue.entries[1])
        );
    }

    #[test]
    fn goes_to_whats_up_next_now_when_the_queue_changed() {
        let queue = after([playing(0), Step::PlayNext { track_ids: vec![1] }]);
        assert_eq!(track(&stepped(&queue, Step::Handoff)), Some(1));
    }

    #[test]
    fn stops_as_a_track_that_ends_does_with_nothing_up_next() {
        let last = after([playing(2)]);
        assert_eq!(stepped(&last, Step::Handoff), stepped(&last, Step::Ended));
    }
}

mod remembering_a_queue {
    use super::*;

    fn restored(saved: Option<&str>) -> Queue {
        Queue::restored(saved, false, Repeat::Off)
    }

    #[test]
    fn remembers_nothing_when_nothings_queued() {
        assert_eq!(Queue::default().remembered(0.0), None);
    }

    #[test]
    fn remembers_tracks_the_current_one_and_the_order_before_shuffling() {
        let queue = after([play(&[10, 20, 30], Some(1), 1)]);
        assert_eq!(
            queue.remembered(0.0).unwrap(),
            r#"{"trackIds":[10,20,30],"current":1,"unshuffled":null,"time":0.0}"#
        );
        let shuffled = after([
            Step::SetShuffle {
                shuffle: true,
                seed: 7,
            },
            play(&[10, 20, 30], Some(0), 7),
        ]);
        let order: Vec<usize> = shuffled
            .unshuffled
            .as_ref()
            .unwrap()
            .iter()
            .map(|entry| shuffled.entries.iter().position(|e| e == entry).unwrap())
            .collect();
        assert_eq!(
            shuffled.remembered(0.0).unwrap(),
            format!(
                r#"{{"trackIds":{:?},"current":0,"unshuffled":{:?},"time":0.0}}"#,
                tracks(&shuffled),
                order
            )
            .replace(' ', "")
        );
    }

    #[test]
    fn comes_back_paused_with_its_tracks_and_its_current_one() {
        let queue = after([
            play(&[10, 20, 10], Some(2), 1),
            Step::Move { key: 2, by: -1 },
        ]);
        let back = restored(queue.remembered(0.0).as_deref());
        assert_eq!(tracks(&back), tracks(&queue));
        assert_eq!(back.current, queue.current);
        assert!(!back.playing);
        // New keys, each once, and the next one after them
        let keys: Vec<u64> = back.entries.iter().map(|entry| entry.key).collect();
        assert_eq!(keys, [1, 2, 3]);
        assert_eq!(back.next_key, 4);
    }

    #[test]
    fn comes_back_shuffled_able_to_unshuffle() {
        let queue = modes(true, Repeat::All, Some(0));
        let back = Queue::restored(queue.remembered(0.0).as_deref(), true, Repeat::All);
        assert_eq!(tracks(&back), tracks(&queue));
        assert_eq!(back.repeat, Repeat::All);
        let unshuffle = Step::SetShuffle {
            shuffle: false,
            seed: 1,
        };
        assert_eq!(
            tracks(&stepped(&back, unshuffle.clone())),
            tracks(&stepped(&queue, unshuffle))
        );
    }

    #[test]
    fn keeps_the_order_it_played_in_when_shuffle_has_been_turned_off_since() {
        let queue = modes(true, Repeat::Off, Some(0));
        assert_eq!(
            tracks(&restored(queue.remembered(0.0).as_deref())),
            tracks(&queue)
        );
    }

    #[test]
    fn comes_back_with_an_order_before_shuffling_when_it_had_none() {
        let back = Queue::restored(
            Some(r#"{"trackIds":[10,20],"current":0,"unshuffled":null}"#),
            true,
            Repeat::Off,
        );
        let before: Vec<Entry> = back.unshuffled.clone().unwrap();
        assert_eq!(before, back.entries);
    }

    #[test]
    fn remembers_the_time_heard_to_the_millisecond() {
        let queue = after([play(&[10, 20], Some(1), 1)]);
        let saved = queue.remembered(83.12345).unwrap();
        assert!(saved.ends_with(r#""time":83.123}"#), "{saved}");
        assert_eq!(queue::restored_time(Some(&saved)), 83.123);
    }

    #[test]
    fn comes_back_at_its_start_without_a_time() {
        // As an older version remembered it, or with a time that isn't one
        for saved in [
            r#"{"trackIds":[10,20],"current":1,"unshuffled":null}"#,
            r#"{"trackIds":[10,20],"current":1,"unshuffled":null,"time":-4}"#,
            r#"{"trackIds":[10,20],"current":1,"unshuffled":null,"time":"1:23"}"#,
        ] {
            assert_eq!(restored(Some(saved)).current, Some(1));
            assert_eq!(queue::restored_time(Some(saved)), 0.0, "{saved}");
        }
        assert_eq!(queue::restored_time(Some("not json")), 0.0);
        assert_eq!(queue::restored_time(None), 0.0);
    }

    #[test]
    fn comes_back_empty_from_anything_that_isnt_a_queue() {
        for saved in [
            None,
            Some("null"),
            Some(r#""queue""#),
            Some("{}"),
            Some("not json"),
            Some(r#"{"trackIds":[],"current":0,"unshuffled":null}"#),
            Some(r#"{"trackIds":[1,2],"current":2,"unshuffled":null}"#),
            Some(r#"{"trackIds":[1,-2],"current":0,"unshuffled":null}"#),
            Some(r#"{"trackIds":[1,2],"current":0,"unshuffled":[0,0]}"#),
            Some(r#"{"trackIds":[1,2],"current":0,"unshuffled":3}"#),
        ] {
            let back = Queue::restored(saved, true, Repeat::One);
            assert_eq!(back.current_entry(), None, "{saved:?}");
            // With the modes it was given
            assert!(back.shuffle);
            assert_eq!(back.repeat, Repeat::One);
        }
    }
}
