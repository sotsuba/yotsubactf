# Bot Setup & Subscriptions

This guide helps you set up YotsubaCTF in your Discord server and explains how the notification system works.

## 1. Invite the Bot
To get started, invite the Bot to your server using an invite link generated from the [Discord Developer Portal](https://discord.com/developers/applications).

**Required Permissions:**
- `View Channels`
- `Send Messages`
- `Embed Links`

## 2. Setting Up Event Notifications
The most important part of YotsubaCTF is staying updated on CTF events. This is done via the **Subscription** system.

### `/subscribe`
Use this command to tell the bot which channel should receive live CTF updates.
- **Command:** `/subscribe channel: #ctf-announcements`
- **What happens:** The bot will post a message whenever a new CTF is discovered or when an event is about to start.

> [!WARNING]
> **Initial "Spam" Warning:** When you first subscribe, Yotsuba might be a bit "mean" and spam several notifications at once. This is because she is catching up and firing off alerts for everything she recently discovered that hasn't been notified yet. Don't worry, she'll settle down after the first burst!

> [!NOTE]
> Only one channel can be subscribed per server. Running the command again in a different channel will move the subscription there.

### `/unsubscribe`
If you want to stop receiving notifications entirely.
- **Command:** `/unsubscribe`

---

## 3. Weekly Digest
If live notifications are too noisy, you can enable a weekly digest that summarizes upcoming events for the week.

### `/digest enable`
- **Command:** `/digest enable day: Monday channel: #general`
- **What happens:** Every Monday, the bot will post a beautiful summary of all CTFs happening that week.

---

## 4. Writeup Notifications
By default, when you `/subscribe` to a channel, you also opt-in to receive writeup notifications. These are posted whenever the bot finds new writeups for CTFs on CTFTime.

### `/writeups notify`
You can toggle these independently of the main event subscription.
- **Command:** `/writeups notify enabled: False` (to disable)

---

## 5. Tracking Your Team
YotsubaCTF can follow a specific team on CTFTime and notify you when new results are posted.

### `/team follow`
- **Command:** `/team follow id: 12345` (Find your team ID in the CTFTime URL: `https://ctftime.org/team/12345`)
- **What happens:** The bot will post your team's rank and score whenever a CTF finishes.

---

## FAQ

**Q: Why isn't the bot posting anything?**
1. Check if the bot has `Send Messages` and `Embed Links` permissions in the subscribed channel.
2. Ensure you have actually run `/subscribe`. You can check the current setup using `/digest status` (as it also shows subscription info).

**Q: Can I have multiple subscription channels?**
No, Yotsuba likes to keep things organized. Only one announcement channel is supported per server.

**Q: Does `/subscribe` include writeups?**
Yes! Event announcements and writeup notifications both go to the same subscribed channel by default.
