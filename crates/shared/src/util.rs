pub fn build_user_agent(email: &str) -> String {
    format!("ctftime-discord-bot/1.0 (contact: {email})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_user_agent_uses_expected_format() {
        let email = "maintainer@example.com";
        let user_agent = build_user_agent(email);
        assert_eq!(
            user_agent,
            "ctftime-discord-bot/1.0 (contact: maintainer@example.com)"
        );
    }
}
