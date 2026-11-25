use proptest::prelude::*;
use router::{parse_golem, parse_mention, preferred_seat_variable, seat_holders, seat_secret};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn only_the_first_line_has_command_authority_and_context_is_lossless(
        golem in "[a-z][a-z0-9_-]{0,23}",
        context in "[a-zé🦀 \t\r\n]{0,128}",
        newline in prop_oneof![Just("\n"), Just("\r\n")],
    ) {
        let instructions = format!("inspect{newline}{context}{newline}@isp-golem-xiv stop");
        let body = format!("@isp-golem-xiv\t{golem}  {instructions}");
        let mention = parse_mention(&body).ok_or_else(|| TestCaseError::fail("valid mention rejected"))?;
        prop_assert_eq!(mention.golem, golem.as_str());
        prop_assert_eq!(mention.instructions, instructions.as_str());
        let quoted = format!("Some context:{newline}{body}");
        prop_assert_eq!(parse_mention(&quoted), None);
    }

    #[test]
    fn mention_size_limit_counts_utf8_bytes_not_characters(crabs in 1usize..4_000) {
        const PREFIX: &str = "@isp-golem-xiv hello ";
        let instructions = "🦀".repeat(crabs) + &"x".repeat(16_384 - PREFIX.len() - crabs * 4);
        let mut body = format!("{PREFIX}{instructions}");
        let mention = parse_mention(&body).ok_or_else(|| TestCaseError::fail("mention at byte limit rejected"))?;
        prop_assert_eq!(mention.instructions, instructions.as_str());
        body.push('x');
        prop_assert_eq!(parse_mention(&body), None);
    }

    #[test]
    fn frontmatter_budget_is_unit_independent_but_duplicate_keys_are_invalid(
        minutes in 1u32..=300,
        newline in prop_oneof![Just("\n"), Just("\r\n")],
    ) {
        let seconds = minutes * 60;
        for timeout in [format!("{minutes}m"), format!("{seconds}s")] {
            let document = ["---", &format!("timeout: {timeout}"), "---", "Inspect."].join(newline);
            let golem = parse_golem("review", &document);
            prop_assert!(golem.errors.is_empty(), "{:?}", golem.errors);
            prop_assert_eq!(golem.timeout_seconds(), Some(seconds));
            prop_assert_eq!(golem.purpose, "Inspect.");
        }
        let duplicate = format!("---{newline}timeout: {minutes}m{newline}timeout: {seconds}s{newline}---{newline}Inspect.");
        prop_assert!(!parse_golem("review", &duplicate).errors.is_empty());

        let over_budget = 300 + minutes;
        let too_long = format!("---{newline}timeout: {over_budget}m{newline}---{newline}Inspect.");
        prop_assert!(!parse_golem("review", &too_long).errors.is_empty());
    }

    #[test]
    fn seat_names_are_case_insensitive_and_resolve_to_the_current_member(
        left in "[a-zA-Z0-9]{1,19}",
        right in "[a-zA-Z0-9]{1,19}",
    ) {
        let login = format!("{left}-{right}");
        let member = login.to_ascii_lowercase();
        let anthropic = seat_secret(&login, "anthropic")
            .ok_or_else(|| TestCaseError::fail("valid Anthropic seat rejected"))?;
        let codex = seat_secret(&login, "openai-codex")
            .ok_or_else(|| TestCaseError::fail("valid Codex seat rejected"))?;
        prop_assert_ne!(&anthropic, &codex);
        let member_anthropic = seat_secret(&member, "anthropic");
        let member_codex = seat_secret(&member, "openai-codex");
        prop_assert_eq!(member_anthropic.as_deref(), Some(anthropic.as_str()));
        prop_assert_eq!(member_codex.as_deref(), Some(codex.as_str()));

        let holders = seat_holders(&[&codex, &anthropic], &[&member]);
        prop_assert_eq!(holders.registered, vec![(member, vec!["anthropic", "openai-codex"])]);
        prop_assert!(holders.departed.is_empty());

        let preferred = format!("SEAT_PREFERRED_{}_{}", left.to_ascii_uppercase(), right.to_ascii_uppercase());
        prop_assert_eq!(preferred_seat_variable(&login), Some(preferred));
        // Already-normalized logins must be refused rather than aliasing a real member.
        let alias = format!("{left}_{right}");
        prop_assert_eq!(seat_secret(&alias, "anthropic"), None);
        prop_assert_eq!(preferred_seat_variable(&alias), None);
    }

    #[test]
    fn seat_login_limit_accepts_39_ascii_characters_and_rejects_40(login in "[a-zA-Z0-9]{39}") {
        prop_assert!(seat_secret(&login, "anthropic").is_some());
        prop_assert!(preferred_seat_variable(&login).is_some());
        let too_long = login + "0";
        prop_assert_eq!(seat_secret(&too_long, "anthropic"), None);
        prop_assert_eq!(preferred_seat_variable(&too_long), None);
    }
}
