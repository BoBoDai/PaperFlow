//! Shared character budgets for paper context and chat transcripts.
//!
//! Q&A and the three-brain features use the same rules so the two paths cannot
//! drift into different truncation behavior.

use paperflow_core::MagiTurn;

pub(crate) fn char_count(text: &str) -> usize {
    text.chars().count()
}

/// Truncate context to `budget` characters.
///
/// When `keep_tail` is true (full-paper scope) the first 3/4 and the last 1/4
/// are kept with a Chinese omission marker in the middle. Otherwise only the
/// head is kept.
pub(crate) fn truncate_context(context: &str, budget: usize, keep_tail: bool) -> (String, bool) {
    if char_count(context) <= budget {
        return (context.to_string(), false);
    }

    if !keep_tail {
        return (context.chars().take(budget).collect(), true);
    }

    let head: String = context.chars().take(budget * 3 / 4).collect();
    let tail: String = context
        .chars()
        .rev()
        .take(budget / 4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    (
        format!("{head}\n\n[…上下文过长，中间内容已省略…]\n\n{tail}"),
        true,
    )
}

/// Keep whole recent turns until the character budget is exhausted.
///
/// The most recent turn is always kept even when it alone exceeds the budget,
/// because dropping it would remove the message the user is asking about.
pub(crate) fn trim_transcript(turns: Vec<MagiTurn>, budget: usize) -> Vec<MagiTurn> {
    let mut kept = Vec::new();
    let mut total = 0;

    for turn in turns.into_iter().rev() {
        let length = char_count(&turn.content);
        if !kept.is_empty() && total + length > budget {
            break;
        }
        total += length;
        kept.push(turn);
        if total >= budget {
            break;
        }
    }

    kept.reverse();
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use paperflow_core::PersonaId;

    #[test]
    fn truncates_head_only_when_tail_is_not_requested() {
        let (text, truncated) = truncate_context("abcdefghij", 4, false);
        assert!(truncated);
        assert_eq!(text, "abcd");
    }

    #[test]
    fn keeps_head_and_tail_for_full_text() {
        let (text, truncated) = truncate_context("abcdefghij", 8, true);
        assert!(truncated);
        assert!(text.starts_with("abcdef"));
        assert!(text.ends_with("ij"));
        assert!(text.contains("中间内容已省略"));
    }

    #[test]
    fn keeps_the_most_recent_turns() {
        let turns = vec![
            MagiTurn {
                persona: None,
                content: "old".into(),
            },
            MagiTurn {
                persona: Some(PersonaId::Rigor),
                content: "middle".into(),
            },
            MagiTurn {
                persona: Some(PersonaId::Repro),
                content: "new".into(),
            },
        ];

        let kept = trim_transcript(turns, 10);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].content, "middle");
        assert_eq!(kept[1].content, "new");
    }

    #[test]
    fn always_keeps_the_latest_turn() {
        let turns = vec![MagiTurn {
            persona: None,
            content: "a very long latest question".into(),
        }];

        let kept = trim_transcript(turns, 4);
        assert_eq!(kept.len(), 1);
    }
}
