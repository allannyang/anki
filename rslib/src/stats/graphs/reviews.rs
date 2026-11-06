// Copyright: Anki Software, LLC and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

use anki_proto::stats::graphs_response::ReviewCountsAndTimes;

use super::GraphsContext;
use crate::revlog::RevlogReviewKind;

impl GraphsContext {
    pub(super) fn review_counts_and_times(&self) -> ReviewCountsAndTimes {
        let mut data = ReviewCountsAndTimes::default();
        for review in &self.revlog {
            if review.review_kind == RevlogReviewKind::Manual
                || review.review_kind == RevlogReviewKind::Rescheduled
            {
                continue;
            }
            let day = self.day_of(review.id.as_secs());
            let count = data.count.entry(day).or_insert_with(Default::default);
            let time = data.time.entry(day).or_insert_with(Default::default);
            match review.review_kind {
                RevlogReviewKind::Learning => {
                    count.learn += 1;
                    time.learn += review.taken_millis;
                }
                RevlogReviewKind::Relearning => {
                    count.relearn += 1;
                    time.relearn += review.taken_millis;
                }
                RevlogReviewKind::Review => {
                    if review.last_interval < 21 {
                        count.young += 1;
                        time.young += review.taken_millis;
                    } else {
                        count.mature += 1;
                        time.mature += review.taken_millis;
                    }
                }
                RevlogReviewKind::Filtered => {
                    count.filtered += 1;
                    time.filtered += review.taken_millis;
                }
                RevlogReviewKind::Manual | RevlogReviewKind::Rescheduled => unreachable!(),
            }
        }
        data
    }
}

#[cfg(test)]
mod test {
    use chrono::DateTime;
    use chrono::FixedOffset;
    use chrono::TimeZone;
    use itertools::Itertools;

    use super::*;
    use crate::prelude::*;
    use crate::revlog::RevlogEntry;
    use crate::scheduler::timing::day_starts;

    fn review_at(dt: DateTime<FixedOffset>) -> RevlogEntry {
        RevlogEntry {
            id: RevlogId(dt.timestamp_millis()),
            review_kind: RevlogReviewKind::Review,
            ..Default::default()
        }
    }

    #[test]
    fn review_counts_keep_early_morning_review_on_its_day_after_clocks_fall_back() {
        let (edt, est) = (Eastern2026::edt(), Eastern2026::est());
        // viewed on the evening clocks fell back
        let now = TimestampSecs(
            est.with_ymd_and_hms(2026, 11, 1, 22, 0, 0)
                .unwrap()
                .timestamp(),
        );
        let ctx = GraphsContext {
            revlog: vec![
                review_at(edt.with_ymd_and_hms(2026, 10, 27, 20, 0, 0).unwrap()),
                // Wednesday's only session, just after the rollover
                review_at(edt.with_ymd_and_hms(2026, 10, 28, 6, 20, 0).unwrap()),
                review_at(est.with_ymd_and_hms(2026, 11, 1, 6, 30, 0).unwrap()),
            ],
            cards: vec![],
            next_day_start: TimestampSecs(
                est.with_ymd_and_hms(2026, 11, 2, 6, 0, 0)
                    .unwrap()
                    .timestamp(),
            ),
            day_starts: day_starts(&Eastern2026, now, 6, -7..=0).unwrap(),
            days_elapsed: 0,
            local_offset_secs: est.local_minus_utc().into(),
        };

        let counts = ctx.review_counts_and_times().count;

        // Tuesday, Wednesday and Sunday
        assert_eq!(
            counts.keys().copied().sorted().collect_vec(),
            vec![-5, -4, 0]
        );
    }
}
