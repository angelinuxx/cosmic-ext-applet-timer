// SPDX-License-Identifier: MIT

//! Editing rules for the preset list shown in the settings popup. Pure
//! functions on the list of minutes, no UI.

pub const MAX_PRESETS: usize = 3;
pub const MIN_MINUTES: u64 = 1;
pub const MAX_MINUTES: u64 = 999;
/// How much longer than the last preset a newly added one is.
const ADD_STEP: u64 = 5;

/// An edit to the preset at a given index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetAction {
    Decrement,
    Increment,
    Set(u64),
    MoveUp,
    MoveDown,
    Remove,
}

impl PresetAction {
    /// Whether the action changes anything; the popup disables the buttons
    /// of actions that would not.
    pub fn allowed(self, presets: &[u64], index: usize) -> bool {
        let Some(&minutes) = presets.get(index) else {
            return false;
        };
        match self {
            Self::Decrement => minutes > MIN_MINUTES,
            Self::Increment => minutes < MAX_MINUTES,
            Self::Set(value) => (MIN_MINUTES..=MAX_MINUTES).contains(&value),
            Self::MoveUp => index > 0,
            Self::MoveDown => index + 1 < presets.len(),
            Self::Remove => presets.len() > 1,
        }
    }

    /// Applies the action; does nothing if it is not allowed.
    pub fn apply(self, presets: &mut Vec<u64>, index: usize) {
        if !self.allowed(presets, index) {
            return;
        }
        match self {
            Self::Decrement => {
                presets[index] = (presets[index] - 1).clamp(MIN_MINUTES, MAX_MINUTES);
            }
            Self::Increment => {
                presets[index] = (presets[index] + 1).clamp(MIN_MINUTES, MAX_MINUTES);
            }
            Self::Set(value) => presets[index] = value,
            Self::MoveUp => presets.swap(index - 1, index),
            Self::MoveDown => presets.swap(index, index + 1),
            Self::Remove => {
                presets.remove(index);
            }
        }
    }
}

pub fn can_add(presets: &[u64]) -> bool {
    presets.len() < MAX_PRESETS
}

/// Appends a preset `ADD_STEP` minutes longer than the last one.
pub fn add(presets: &mut Vec<u64>) {
    if !can_add(presets) {
        return;
    }
    let minutes = presets
        .last()
        .map_or(ADD_STEP, |last| last + ADD_STEP)
        .clamp(MIN_MINUTES, MAX_MINUTES);
    presets.push(minutes);
}

/// Reads the minutes typed into a preset field; `None` if the text is not a
/// whole number in range.
pub fn parse_minutes(text: &str) -> Option<u64> {
    text.trim()
        .parse()
        .ok()
        .filter(|minutes| (MIN_MINUTES..=MAX_MINUTES).contains(minutes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(mut presets: Vec<u64>, index: usize, action: PresetAction) -> Vec<u64> {
        action.apply(&mut presets, index);
        presets
    }

    #[test]
    fn increment_and_decrement_stay_in_range() {
        assert_eq!(applied(vec![5], 0, PresetAction::Increment), vec![6]);
        assert_eq!(applied(vec![5], 0, PresetAction::Decrement), vec![4]);
        assert_eq!(applied(vec![1], 0, PresetAction::Decrement), vec![1]);
        assert_eq!(applied(vec![999], 0, PresetAction::Increment), vec![999]);
    }

    #[test]
    fn out_of_range_value_from_the_file_is_pulled_back() {
        assert_eq!(applied(vec![0], 0, PresetAction::Increment), vec![1]);
        assert_eq!(applied(vec![2000], 0, PresetAction::Decrement), vec![999]);
    }

    #[test]
    fn set_rejects_out_of_range_values() {
        assert_eq!(applied(vec![5], 0, PresetAction::Set(20)), vec![20]);
        assert_eq!(applied(vec![5], 0, PresetAction::Set(0)), vec![5]);
        assert_eq!(applied(vec![5], 0, PresetAction::Set(1000)), vec![5]);
    }

    #[test]
    fn move_swaps_with_the_neighbour() {
        assert_eq!(
            applied(vec![1, 2, 3], 1, PresetAction::MoveUp),
            vec![2, 1, 3]
        );
        assert_eq!(
            applied(vec![1, 2, 3], 1, PresetAction::MoveDown),
            vec![1, 3, 2]
        );
    }

    #[test]
    fn move_past_the_ends_does_nothing() {
        assert_eq!(applied(vec![1, 2], 0, PresetAction::MoveUp), vec![1, 2]);
        assert_eq!(applied(vec![1, 2], 1, PresetAction::MoveDown), vec![1, 2]);
        assert!(!PresetAction::MoveUp.allowed(&[1, 2], 0));
        assert!(!PresetAction::MoveDown.allowed(&[1, 2], 1));
    }

    #[test]
    fn remove_keeps_at_least_one_preset() {
        assert_eq!(applied(vec![1, 2], 0, PresetAction::Remove), vec![2]);
        assert_eq!(applied(vec![1], 0, PresetAction::Remove), vec![1]);
    }

    #[test]
    fn invalid_index_is_ignored() {
        assert_eq!(applied(vec![1], 3, PresetAction::Remove), vec![1]);
        assert!(!PresetAction::Increment.allowed(&[1], 3));
    }

    #[test]
    fn add_appends_last_plus_step_up_to_the_maximum_count() {
        let mut presets = vec![10];
        add(&mut presets);
        assert_eq!(presets, vec![10, 15]);

        let mut empty = Vec::new();
        add(&mut empty);
        assert_eq!(empty, vec![5]);

        let mut full = vec![1, 2, 3];
        assert!(!can_add(&full));
        add(&mut full);
        assert_eq!(full, vec![1, 2, 3]);
    }

    #[test]
    fn add_never_exceeds_the_maximum_minutes() {
        let mut presets = vec![997];
        add(&mut presets);
        assert_eq!(presets, vec![997, 999]);
    }

    #[test]
    fn parse_accepts_whole_numbers_in_range() {
        assert_eq!(parse_minutes("15"), Some(15));
        assert_eq!(parse_minutes(" 7 "), Some(7));
        assert_eq!(parse_minutes("0"), None);
        assert_eq!(parse_minutes("1000"), None);
        assert_eq!(parse_minutes(""), None);
        assert_eq!(parse_minutes("abc"), None);
        assert_eq!(parse_minutes("1.5"), None);
        assert_eq!(parse_minutes("-3"), None);
    }
}
