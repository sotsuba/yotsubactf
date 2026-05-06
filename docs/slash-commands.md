# Slash Commands

This document lists the available slash commands and what they do.

## General

| Command | Description | Options |
| --- | --- | --- |
| `/help` | Show the command list. | - |
| `/ping` | Check bot responsiveness. | - |

## Events

| Command | Description | Options |
| --- | --- | --- |
| `/event upcoming` | List upcoming CTFs. | `count` (1-25), `format` (Jeopardy, Attack-Defense, Mixed), `weight_min`, `weight_max`, `onsite`, `sort_by` (time, weight) |
| `/event current` | List CTFs currently running. | `count` (1-25) |
| `/event completed` | List recently ended CTFs. | `count` (1-25), `format`, `weight_min` |
| `/event countdown` | Countdown to a CTF by name. | `query` (required) |
| `/event info` | Details for a CTF by name. | `query` (required) |

## Notifications

| Command | Description | Options |
| --- | --- | --- |
| `/subscribe` | Subscribe this server to event notifications. | `channel` (required, text channel) |
| `/unsubscribe` | Stop receiving event notifications. | - |

## Writeups

| Command | Description | Options |
| --- | --- | --- |
| `/writeups search` | Search writeups by keyword. | `query` (required), `category` (web, pwn, crypto, forensics, rev, misc, osint) |
| `/writeups recent` | Show most recent writeups. | - |
| `/writeups event` | Browse writeups for a CTF event. | `name` (required, partial match) |
| `/writeups category` | Browse writeups by category. | `name` (required) |
| `/writeups team` | Writeups for the followed team in this guild. | - |

## Teams and Results

| Command | Description | Options |
| --- | --- | --- |
| `/team search` | Search for a CTFtime team. | `name` (required) |
| `/team follow` | Track a team by CTFtime ID (1 per guild). | `id` (required) |
| `/team unfollow` | Stop tracking the current team. | - |
| `/team following` | Show the team tracked by this guild. | - |
| `/leaderboard` | Show the CTFtime leaderboard. | `year` (defaults to current year) |

## Reminders

| Command | Description | Options |
| --- | --- | --- |
| `/reminder set event` | Reminder before a CTF event. | `event_id` (required), `days`, `hours`, `minutes` |
| `/reminder set timer` | One-off countdown reminder. | `days`, `hours`, `minutes`, `message` |
| `/reminder set recurring` | Recurring reminder. | `for_hours` (required), `every_minutes` (required), `delay_minutes`, `message` |
| `/reminder list` | List active reminders. | - |
| `/reminder cancel` | Cancel a reminder. | `number` (required, from list) |

## Digest

| Command | Description | Options |
| --- | --- | --- |
| `/digest enable` | Enable the weekly digest. | `day` (required), `channel` (required) |
| `/digest disable` | Disable the weekly digest. | - |
| `/digest status` | Show current digest status. | - |

## Admin

| Command | Description | Options |
| --- | --- | --- |
| `/adminrole add` | Grant an admin role mapping. | `role_id` (required), `level` (required: owner, admin, moderator, analyst) |
| `/adminrole remove` | Remove an admin role mapping. | `role_id` (required) |
| `/adminrole list` | List admin role mappings. | - |

## Utilities

| Command | Description | Options |
| --- | --- | --- |
| `/cipher` | Encode or decode a string. | `type` (required: base64, base32, hex, url, rot13, binary, morse, atbash), `mode` (required: encode, decode), `input` (required) |
| `/hash` | Compute a cryptographic hash. | `type` (required: md5, sha1, sha256, sha512), `input` (required) |
