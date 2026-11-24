// Copyright: Anki Software, LLC and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

use anki_proto::stats::graphs_response::Today;

use crate::revlog::RevlogReviewKind;
use crate::stats::graphs::GraphsContext;

impl GraphsContext {
    pub(super) fn today(&self) -> Today {
        let mut today = Today::default();
        let start_of_today_ms = self.start_of_last_days(1).as_millis().0;
        for review in self.revlog.iter().rev() {
            if review.id.0 < start_of_today_ms {
                continue;
            }
            if review.review_kind == RevlogReviewKind::Manual
                || review.review_kind == RevlogReviewKind::Rescheduled
            {
                continue;
            }
            // total
            today.answer_count += 1;
            today.answer_millis += review.taken_millis;
            // correct
            if review.button_chosen > 1 {
                today.correct_count += 1;
            }
            // mature
            if review.last_interval >= 21 {
                today.mature_count += 1;
                if review.button_chosen > 1 {
                    today.mature_correct += 1;
                }
            }
            // type counts
            match review.review_kind {
                RevlogReviewKind::Learning => today.learn_count += 1,
                RevlogReviewKind::Review => today.review_count += 1,
                RevlogReviewKind::Relearning => today.relearn_count += 1,
                RevlogReviewKind::Filtered => today.early_review_count += 1,
                RevlogReviewKind::Manual | RevlogReviewKind::Rescheduled => unreachable!(),
            }
        }
        today
    }
}

#[cfg(test)]
mod test {
    use chrono::TimeZone;

    use super::*;
    use crate::prelude::*;
    use crate::revlog::RevlogEntry;

    #[test]
    fn today_counts_review_from_first_hour_of_25_hour_day() {
        let (edt, est) = (Eastern2026::edt(), Eastern2026::est());
        // with a midnight rollover, the day clocks fall back is 25 hours long
        let now = est.with_ymd_and_hms(2026, 11, 1, 22, 0, 0).unwrap();
        let just_after_midnight = edt.with_ymd_and_hms(2026, 11, 1, 0, 30, 0).unwrap();
        let revlog = vec![RevlogEntry {
            id: RevlogId(just_after_midnight.timestamp_millis()),
            review_kind: RevlogReviewKind::Review,
            ..Default::default()
        }];
        let ctx = GraphsContext::eastern_2026(now, 0, revlog, vec![]);

        let today = ctx.today();

        assert_eq!(today.answer_count, 1);
    }
}
