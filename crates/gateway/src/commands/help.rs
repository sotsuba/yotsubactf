use async_trait::async_trait;
use shared::CtfResult;
use twilight_model::application::command::CommandType;
use twilight_model::http::interaction::InteractionResponse;
use twilight_util::builder::command::CommandBuilder;

use super::{CommandContext, SlashCommand};
use crate::embed::{CtfEmbed, ephemeral_embed};

pub struct HelpCommand;

#[async_trait]
impl SlashCommand for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }
    fn definition(&self) -> twilight_model::application::command::Command {
        CommandBuilder::new("help", "Show bot commands", CommandType::ChatInput).build()
    }
    async fn handle(&self, _ctx: CommandContext<'_>) -> CtfResult<InteractionResponse> {
        Ok(handle())
    }
}

pub fn handle() -> InteractionResponse {
    let embed = CtfEmbed::new("YotsubaCTF — Commands")
        .description(
            "### 🏁 CTF Operations\n\
             **`/event upcoming`** — List upcoming CTFs\n\
             **`/event current`** — List ongoing CTFs\n\
             **`/event completed`** — Browse past results\n\
             **`/writeups`** — Search and browse CTF writeups\n\
             **`/team follow`** — Get notified of new team results\n\
             **`/leaderboard`** — Check top teams on CTFTime\n\n\
             ### ⏰ Reminders\n\
             **`/reminder set`** — Set a reminder for an event or timer\n\
             **`/reminder list`** — View and manage your active reminders\n\n\
             ### 🛠️ Utilities\n\
             **`/cipher`** — Cipher tools (ROT13, Base64, etc.)\n\
             **`/hash`** — Calculate common hash values\n\
             **`/ping`** — Check bot responsiveness\n\n\
             ### ⚙️ Administration\n\
             **`/subscribe`** — Configure notification channels\n\
             **`/unsubscribe`** — Stop notifications\n\
             **`/digest`** — Configure weekly CTF digests\n\
             **`/adminrole`** — Manage admin role mappings\n\n\
             **`/help`** — Show this message",
        )
        .now()
        .build();

    ephemeral_embed(embed)
}
