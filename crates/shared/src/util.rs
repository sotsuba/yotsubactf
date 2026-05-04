pub fn build_user_agent(email: &str) -> String {
    format!("ctftime-discord-bot/1.0 (contact: {email})")
}
