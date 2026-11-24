// Copyright: Anki Software, LLC and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

use anki_proto::stats::graphs_response::Added;

use super::GraphsContext;

impl GraphsContext {
    pub(super) fn added_days(&self) -> Added {
        let mut data = Added::default();
        for card in &self.cards {
            let day = self.day_of(card.id.as_secs());
            *data.added.entry(day).or_insert_with(Default::default) += 1;
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

    fn card_added_at(dt: DateTime<FixedOffset>) -> Card {
        Card {
            id: CardId(dt.timestamp_millis()),
            ..Default::default()
        }
    }

    #[test]
    fn added_days_keep_early_morning_card_on_its_day_after_clocks_fall_back() {
        let (edt, est) = (Eastern2026::edt(), Eastern2026::est());
        // viewed on the evening clocks fell back
        let now = est.with_ymd_and_hms(2026, 11, 1, 22, 0, 0).unwrap();
        let cards = vec![
            card_added_at(edt.with_ymd_and_hms(2026, 10, 27, 20, 0, 0).unwrap()),
            // Wednesday's only card, just after the rollover
            card_added_at(edt.with_ymd_and_hms(2026, 10, 28, 6, 20, 0).unwrap()),
            card_added_at(est.with_ymd_and_hms(2026, 11, 1, 6, 30, 0).unwrap()),
        ];
        let ctx = GraphsContext::eastern_2026(now, 6, vec![], cards);

        let added = ctx.added_days().added;

        // Tuesday, Wednesday and Sunday
        assert_eq!(
            added.keys().copied().sorted().collect_vec(),
            vec![-5, -4, 0]
        );
    }
}
