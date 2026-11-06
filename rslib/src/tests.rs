// Copyright: Anki Software, LLC and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

#![cfg(test)]
#![allow(dead_code)]

use chrono::Duration;
use chrono::FixedOffset;
use chrono::MappedLocalTime;
use chrono::NaiveDate;
use chrono::NaiveDateTime;
use chrono::TimeZone;
use itertools::Itertools;
use tempfile::tempdir;
use tempfile::TempDir;

use crate::collection::CollectionBuilder;
use crate::deckconfig::DeckConfigInner;
use crate::media::MediaManager;
use crate::prelude::*;

pub(crate) fn open_fs_test_collection(name: &str) -> (Collection, TempDir) {
    let tempdir = tempdir().unwrap();
    let dir = tempdir.path();
    let col = CollectionBuilder::new(dir.join(format!("{name}.anki2")))
        .with_desktop_media_paths()
        .build()
        .unwrap();
    (col, tempdir)
}

pub(crate) fn open_test_collection_with_learning_card() -> Collection {
    let mut col = Collection::new();
    NoteAdder::basic(&mut col).add(&mut col);
    col.answer_again();
    col.clear_study_queues();
    col
}

pub(crate) fn open_test_collection_with_relearning_card() -> Collection {
    let mut col = Collection::new();
    NoteAdder::basic(&mut col).add(&mut col);
    col.answer_easy();
    col.storage
        .db
        .execute_batch("UPDATE cards SET due = 0")
        .unwrap();
    col.clear_study_queues();
    col.answer_again();
    col.clear_study_queues();
    col
}

impl Collection {
    pub(crate) fn new() -> Collection {
        CollectionBuilder::default().build().unwrap()
    }

    pub(crate) fn add_media(&self, media: &[(&str, &[u8])]) {
        let mgr = MediaManager::new(&self.media_folder, &self.media_db).unwrap();
        for (name, data) in media {
            mgr.add_file(name, data).unwrap();
        }
    }

    pub(crate) fn get_all_notes(&mut self) -> Vec<Note> {
        self.storage.get_all_notes()
    }

    pub(crate) fn get_first_card(&self) -> Card {
        self.storage.get_all_cards().pop().unwrap()
    }

    pub(crate) fn set_default_learn_steps(&mut self, steps: Vec<f32>) {
        self.update_default_deck_config(|config| config.learn_steps = steps);
    }

    pub(crate) fn set_default_relearn_steps(&mut self, steps: Vec<f32>) {
        self.update_default_deck_config(|config| config.relearn_steps = steps);
    }

    /// Updates with the modified config, then resorts and adjusts remaining
    /// steps in the default deck.
    pub(crate) fn update_default_deck_config(
        &mut self,
        modifier: impl FnOnce(&mut DeckConfigInner),
    ) {
        let config = self
            .get_deck_config(DeckConfigId(1), false)
            .unwrap()
            .unwrap();
        let mut new_config = config.clone();

        modifier(&mut new_config.inner);

        self.update_deck_config_inner(&mut new_config, config.clone(), None)
            .unwrap();
        self.sort_deck(DeckId(1), config.inner.new_card_insert_order(), Usn(0))
            .unwrap();
        self.adjust_remaining_steps_in_deck(DeckId(1), Some(&config), Some(&new_config), Usn(0))
            .unwrap();
    }

    pub(crate) fn basic_notetype(&self) -> Notetype {
        let ntid = self.storage.get_notetype_id("Basic").unwrap().unwrap();
        self.storage.get_notetype(ntid).unwrap().unwrap()
    }

    pub(crate) fn basic_rev_notetype(&self) -> Notetype {
        let ntid = self
            .storage
            .get_notetype_id("Basic (and reversed card)")
            .unwrap()
            .unwrap();
        self.storage.get_notetype(ntid).unwrap().unwrap()
    }

    pub(crate) fn basic_optional_rev_notetype(&self) -> Notetype {
        let ntid = self
            .storage
            .get_notetype_id("Basic (optional reversed card)")
            .unwrap()
            .unwrap();
        self.storage.get_notetype(ntid).unwrap().unwrap()
    }

    pub(crate) fn cloze_notetype(&self) -> Notetype {
        let ntid = self.storage.get_notetype_id("Cloze").unwrap().unwrap();
        self.storage.get_notetype(ntid).unwrap().unwrap()
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct DeckAdder {
    name: NativeDeckName,
    filtered: bool,
    config: Option<DeckConfig>,
}

impl DeckAdder {
    pub(crate) fn new(human_name: impl AsRef<str>) -> Self {
        Self {
            name: NativeDeckName::from_human_name(human_name),
            ..Default::default()
        }
    }

    pub(crate) fn filtered(mut self, filtered: bool) -> Self {
        self.filtered = filtered;
        self
    }

    pub(crate) fn with_config(mut self, modifier: impl FnOnce(&mut DeckConfig)) -> Self {
        let mut config = DeckConfig::default();
        modifier(&mut config);
        self.config = Some(config);
        self
    }

    pub(crate) fn add(mut self, col: &mut Collection) -> Deck {
        let config_opt = self.config.take();
        let mut deck = self.deck();
        if let Some(mut config) = config_opt {
            col.add_or_update_deck_config(&mut config).unwrap();
            deck.normal_mut()
                .expect("can't set config for filtered deck")
                .config_id = config.id.0;
        }
        col.add_or_update_deck(&mut deck).unwrap();
        deck
    }

    pub(crate) fn deck(self) -> Deck {
        let mut deck = if self.filtered {
            Deck::new_filtered()
        } else {
            Deck::new_normal()
        };
        deck.name = self.name;
        deck
    }
}

#[derive(Debug, Clone)]
pub(crate) struct NoteAdder {
    note: Note,
    deck: DeckId,
}

impl NoteAdder {
    pub(crate) fn new(notetype: &Notetype) -> Self {
        Self {
            note: notetype.new_note(),
            deck: DeckId(1),
        }
    }

    pub(crate) fn basic(col: &mut Collection) -> Self {
        Self::new(&col.basic_notetype())
    }

    pub(crate) fn cloze(col: &mut Collection) -> Self {
        Self::new(&col.cloze_notetype())
    }

    pub(crate) fn fields(mut self, fields: &[&str]) -> Self {
        *self.note.fields_mut() = fields.iter().map(ToString::to_string).collect();
        self
    }

    pub(crate) fn deck(mut self, deck: DeckId) -> Self {
        self.deck = deck;
        self
    }

    pub(crate) fn add(mut self, col: &mut Collection) -> Note {
        col.add_note(&mut self.note, self.deck).unwrap();
        self.note
    }

    pub(crate) fn note(self) -> Note {
        self.note
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CardAdder {
    siblings: usize,
    deck: DeckId,
    due_dates: Vec<&'static str>,
}

impl CardAdder {
    pub(crate) fn new() -> Self {
        Self {
            siblings: 1,
            deck: DeckId(1),
            due_dates: Vec::new(),
        }
    }

    pub(crate) fn siblings(mut self, siblings: usize) -> Self {
        self.siblings = siblings;
        self
    }

    pub(crate) fn deck(mut self, deck: DeckId) -> Self {
        self.deck = deck;
        self
    }

    /// Takes an array of strs and sets the due date of the first siblings
    /// accordingly, skipping siblings if a str is empty.
    pub(crate) fn due_dates(mut self, due_dates: impl Into<Vec<&'static str>>) -> Self {
        self.due_dates = due_dates.into();
        self
    }

    pub(crate) fn add(&self, col: &mut Collection) -> Vec<Card> {
        let field = (1..self.siblings + 1)
            .map(|n| format!("{{{{c{n}::}}}}"))
            .join("");
        let note = NoteAdder::cloze(col)
            .fields(&[&field, ""])
            .deck(self.deck)
            .add(col);

        if !self.due_dates.is_empty() {
            let cids = col.storage.card_ids_of_notes(&[note.id]).unwrap();
            for (ord, due_date) in self.due_dates.iter().enumerate() {
                if !due_date.is_empty() {
                    col.set_due_date(&cids[ord..ord + 1], due_date, None)
                        .unwrap();
                }
            }
        }

        col.storage.all_cards_of_note(note.id).unwrap()
    }
}

/// Eastern time in 2026, where clocks spring forward at 2am on Mar 8 and fall
/// back at 2am on Nov 1.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Eastern2026;

impl Eastern2026 {
    pub(crate) fn est() -> FixedOffset {
        FixedOffset::west_opt(5 * 3600).unwrap()
    }

    pub(crate) fn edt() -> FixedOffset {
        FixedOffset::west_opt(4 * 3600).unwrap()
    }
}

impl TimeZone for Eastern2026 {
    type Offset = FixedOffset;

    fn from_offset(_offset: &FixedOffset) -> Self {
        Eastern2026
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<FixedOffset> {
        self.offset_from_local_datetime(&local.and_hms_opt(0, 0, 0).unwrap())
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        // skipped times match neither offset, and repeated times match both
        let matching = [Self::edt(), Self::est()]
            .into_iter()
            .filter(|offset| {
                let utc = *local - Duration::seconds(offset.local_minus_utc().into());
                self.offset_from_utc_datetime(&utc) == *offset
            })
            .collect_vec();
        match matching[..] {
            [offset] => MappedLocalTime::Single(offset),
            [earlier, later] => MappedLocalTime::Ambiguous(earlier, later),
            _ => MappedLocalTime::None,
        }
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        self.offset_from_utc_datetime(&utc.and_hms_opt(0, 0, 0).unwrap())
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        // 2026-03-08 07:00 and 2026-11-01 06:00 UTC
        const SPRING_FORWARD: i64 = 1_772_953_200;
        const FALL_BACK: i64 = 1_793_512_800;
        if (SPRING_FORWARD..FALL_BACK).contains(&utc.and_utc().timestamp()) {
            Self::edt()
        } else {
            Self::est()
        }
    }
}
