pub mod cipher;
pub mod digest;
pub mod event;
pub mod hash;
pub mod help;
pub mod leaderboard;
pub mod admin_roles;
pub mod ping;
pub mod reminder;
pub mod subscribe;
pub mod team;
pub mod unsubscribe;
pub mod writeups;

use crate::state::AppState;
use async_trait::async_trait;
use shared::{AdminRole, CtfResult};
use std::collections::HashMap;
use std::sync::Arc;
use twilight_http::Client as HttpClient;
use twilight_model::application::command::Command;
use twilight_model::application::interaction::application_command::CommandDataOption;
use twilight_model::http::interaction::InteractionResponse;
use twilight_model::id::Id;
use twilight_model::id::marker::{ApplicationMarker, GuildMarker, UserMarker};

#[async_trait]
pub trait SlashCommand: Send + Sync {
    fn name(&self) -> &'static str;
    fn requires_guild(&self) -> bool {
        false
    }
    fn requires_manage_guild(&self) -> bool {
        false
    }
    fn required_admin_role(&self) -> Option<AdminRole> {
        None
    }
    fn definition(&self) -> Command;
    async fn handle(&self, ctx: CommandContext<'_>) -> CtfResult<InteractionResponse>;
    async fn autocomplete(
        &self,
        _ctx: CommandContext<'_>,
    ) -> CtfResult<Option<InteractionResponse>> {
        Ok(None)
    }
}

pub struct CommandContext<'a> {
    pub state: &'a AppState,
    pub guild_id: Option<&'a str>,
    #[allow(dead_code)]
    pub user_id: Id<UserMarker>,
    pub options: &'a [CommandDataOption],
}

pub struct CommandRegistry {
    commands: HashMap<&'static str, Arc<dyn SlashCommand>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut commands: HashMap<&'static str, Arc<dyn SlashCommand>> = HashMap::new();

        let list: Vec<Arc<dyn SlashCommand>> = vec![
            Arc::new(ping::PingCommand),
            Arc::new(help::HelpCommand),
            Arc::new(event::EventCommand),
            Arc::new(cipher::CipherCommand),
            Arc::new(hash::HashCommand),
            Arc::new(leaderboard::LeaderboardCommand),
            Arc::new(writeups::WriteupsCommand),
            Arc::new(team::TeamCommand),
            Arc::new(admin_roles::AdminRoleCommand),
            Arc::new(subscribe::SubscribeCommand),
            Arc::new(unsubscribe::UnsubscribeCommand),
            Arc::new(digest::DigestCommand),
            Arc::new(reminder::ReminderCommand),
        ];

        for cmd in list {
            commands.insert(cmd.name(), cmd);
        }

        Self { commands }
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn SlashCommand>> {
        self.commands.get(name)
    }

    pub fn definitions(&self) -> Vec<Command> {
        self.commands.values().map(|cmd| cmd.definition()).collect()
    }
}

pub async fn register(
    http: &HttpClient,
    application_id: Id<ApplicationMarker>,
    guild_id: Option<Id<GuildMarker>>,
    registry: &CommandRegistry,
) -> anyhow::Result<()> {
    let commands = registry.definitions();

    if let Some(guild_id) = guild_id {
        // Clear global commands to avoid duplicates if they exist from a previous non-guild run
        if let Err(err) = http
            .interaction(application_id)
            .set_global_commands(&[])
            .await
        {
            tracing::warn!(?err, "failed to clear global commands");
        }

        match http
            .interaction(application_id)
            .set_guild_commands(guild_id, &commands)
            .await
        {
            Ok(_) => tracing::info!(?guild_id, "set_guild_commands succeeded"),
            Err(err) => tracing::error!(?err, ?guild_id, "set_guild_commands failed"),
        }
    } else {
        match http
            .interaction(application_id)
            .set_global_commands(&commands)
            .await
        {
            Ok(_) => tracing::info!("set_global_commands succeeded"),
            Err(err) => tracing::error!(?err, "set_global_commands failed"),
        }
    }

    Ok(())
}
