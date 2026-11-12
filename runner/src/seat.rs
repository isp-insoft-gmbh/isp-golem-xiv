//! The paying seat's credential and its encryption for the ops repository's Actions secrets.

use crate::{OutputGuard, Result, codex::CodexAuth};
use base64::{Engine as _, engine::general_purpose};

pub enum Seat {
    /// A Claude subscription setup token; never changes during a run.
    Token(String),
    /// A refreshing Codex sign-in; the run writes back what Codex refreshed.
    Codex(CodexAuth),
}

impl Seat {
    /// # Errors
    /// Rejects values that are not the provider's sign-in format.
    pub fn parse(provider: &str, value: &str) -> Result<Self> {
        match provider {
            "anthropic" => {
                let token = value.trim();
                if !token.starts_with("sk-ant-oat01-")
                    || token.len() < 32
                    || token.len() > 4096
                    || token.bytes().any(|byte| byte.is_ascii_whitespace())
                {
                    return Err("selected seat is not a Claude subscription setup token");
                }
                Ok(Self::Token(token.to_owned()))
            }
            "openai-codex" => CodexAuth::parse(value).map(Self::Codex),
            _ => Err("unsupported seat provider"),
        }
    }

    pub fn guard(&self, guard: &mut OutputGuard) {
        match self {
            Self::Token(token) => guard.add(token),
            Self::Codex(auth) => auth.guard(guard),
        }
    }
}

/// Name of a seat secret the runner may overwrite: `SEAT_<LOGIN>__<PROVIDER>` only.
#[must_use]
pub fn writable_secret_name(name: &str) -> bool {
    name.len() <= 100
        && name.strip_prefix("SEAT_").is_some_and(|rest| {
            rest.contains("__")
                && rest
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

/// libsodium sealed box to the repository's Actions public key, as GitHub requires.
///
/// # Errors
/// Rejects a malformed public key or an encryption failure.
pub fn seal(public_key_base64: &str, plaintext: &[u8]) -> Result<String> {
    let key: [u8; 32] = general_purpose::STANDARD
        .decode(public_key_base64)
        .ok()
        .and_then(|key| key.try_into().ok())
        .ok_or("invalid repository public key")?;
    let mut sealed = vec![0; plaintext.len() + dryoc::constants::CRYPTO_BOX_SEALBYTES];
    dryoc::classic::crypto_box::crypto_box_seal(&mut sealed, plaintext, &key)
        .map_err(|_| "cannot encrypt seat sign-in")?;
    Ok(general_purpose::STANDARD.encode(sealed))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // Generated once with libsodium 1.0.20 (libsodium-wrappers 0.7.15): crypto_box_seed_keypair
    // of 32 bytes 0x07, then crypto_box_seal of MESSAGE. Invented test key, never a real one.
    const PUBLIC: &str = "dh2I7IMEE5Gd/p1NHVbxfmU8jJlAgt9bE3uQoK5u33Q=";
    const SECRET: &str = "L605/v1/o+IAqcYm7vWZ5hotBVxIqCiKTn5MS8o5KPg=";
    const LIBSODIUM_SEALED: &str = "T5qrk+G+POjq61vXyjXHI+99nDdM+6lhYNTBpesQ1zzvSamjt2OR3ZRgpKZcZVFxBDSpAJVTzphtm6+xF+RojV1IpjV+CUeW0QM6ug5XomeuFPn/u7U=";
    const MESSAGE: &[u8] = br#"{"tokens":{"refresh_token":"fixture"}}"#;

    fn open(sealed_base64: &str) -> Option<Vec<u8>> {
        let key = |text: &str| -> [u8; 32] {
            general_purpose::STANDARD
                .decode(text)
                .unwrap()
                .try_into()
                .unwrap()
        };
        let sealed = general_purpose::STANDARD.decode(sealed_base64).ok()?;
        let mut message = vec![
            0;
            sealed
                .len()
                .checked_sub(dryoc::constants::CRYPTO_BOX_SEALBYTES)?
        ];
        dryoc::classic::crypto_box::crypto_box_seal_open(
            &mut message,
            &sealed,
            &key(PUBLIC),
            &key(SECRET),
        )
        .ok()?;
        Some(message)
    }

    #[test]
    fn libsodium_sealed_box_opens_with_the_matching_secret_key() {
        assert_eq!(open(LIBSODIUM_SEALED).unwrap(), MESSAGE);
    }

    #[test]
    fn sealed_sign_in_opens_only_with_the_matching_secret_key_and_is_randomized() {
        let first = seal(PUBLIC, MESSAGE).unwrap();
        let second = seal(PUBLIC, MESSAGE).unwrap();
        assert_ne!(first, second, "ephemeral key per seal");
        assert_eq!(open(&first).unwrap(), MESSAGE);
        assert_eq!(open(&second).unwrap(), MESSAGE);
        let mut tampered = general_purpose::STANDARD.decode(&first).unwrap();
        if let Some(byte) = tampered.last_mut() {
            *byte ^= 1;
        }
        assert!(open(&general_purpose::STANDARD.encode(tampered)).is_none());
        let other = general_purpose::STANDARD.encode([9_u8; 32]);
        assert!(open(&seal(&other, MESSAGE).unwrap()).is_none());
        for invalid in [
            "",
            "not base64",
            "AAAA",
            &general_purpose::STANDARD.encode([1_u8; 31]),
        ] {
            assert!(seal(invalid, MESSAGE).is_err(), "{invalid}");
        }
    }

    #[test]
    fn only_seat_secret_names_are_writable() {
        for (name, writable) in [
            ("SEAT_TEST_MEMBER__OPENAI_CODEX", true),
            ("SEAT_A__ANTHROPIC", true),
            ("APP_PRIVATE_KEY", false),
            ("SEAT_lower__OPENAI_CODEX", false),
            ("SEAT_NOSEPARATOR", false),
            ("SEAT_A__B/../C", false),
            ("SEAT_PREFERRED_A", false),
        ] {
            assert_eq!(writable_secret_name(name), writable, "{name}");
        }
        assert!(!writable_secret_name(&format!(
            "SEAT_A__{}",
            "B".repeat(100)
        )));
    }

    #[test]
    fn seat_parsing_follows_the_provider() {
        let token = format!("sk-ant-oat01-{}", "x".repeat(40));
        assert!(
            matches!(Seat::parse("anthropic", &format!(" {token}\n")), Ok(Seat::Token(t)) if t == token)
        );
        assert!(Seat::parse("openai-codex", &token).is_err());
        assert!(Seat::parse("anthropic", "{}").is_err());
        assert!(Seat::parse("unknown-provider", &token).is_err());
        let auth = crate::codex::tests::auth_json("refresh-token-one", "2026-11-01T00:00:00Z");
        assert!(
            auth.len() > 4096,
            "realistic sign-ins exceed a setup token's bound"
        );
        assert!(matches!(
            Seat::parse("openai-codex", &auth),
            Ok(Seat::Codex(_))
        ));
        assert!(Seat::parse("anthropic", &auth).is_err());
    }
}
