//! Background jobs for friends: refresh the profile and friend list (which
//! also tells the Arctic server the launcher is open), and each action.

use arctic_core::auth::Account;
use arctic_core::friends::{self, InviteTo, Overview, Profile, Settings};
use arctic_core::{Result, cosmetics};

use crate::tasks::{Event, Tasks};

/// Something the player did on the Friends page.
#[derive(Debug, Clone)]
pub enum FriendAction {
    Request(String),
    Accept(String),
    /// Decline, cancel or unfriend.
    Remove(String),
    Invite(String, InviteTo),
    Dismiss(String),
    Update(Option<String>, Settings),
    /// Link another of the player's own accounts into their profile.
    Link(Account),
    Unlink(String),
    /// Make a recovery code (shown once; offline accounts only).
    #[cfg(feature = "offline-accounts")]
    NewRecovery,
    /// Bring an offline profile to this PC with its recovery code.
    Recover(String),
}

impl Tasks {
    fn arctic_token(&self, account: Account) -> Result<(String, String)> {
        let account = self.fresh_account(account)?;
        let base = cosmetics::base_url();
        let token = cosmetics::token_for(self.dirs(), &base, &account)?;
        Ok((base, token))
    }

    /// The profile and friends; also "the launcher is open".
    pub fn friends_refresh(&self, account: Account) {
        self.run(move |t| {
            let result = (|| {
                let (base, token) = t.arctic_token(account)?;
                friends::presence(&base, &token)?;
                Ok::<_, arctic_core::Error>((
                    friends::profile(&base, &token)?,
                    friends::overview(&base, &token)?,
                ))
            })()
            .map_err(|e| e.to_string());
            t.send(Event::Friends(result));
        });
    }

    pub fn friends_action(&self, account: Account, action: FriendAction) {
        self.run(move |t| {
            match &action {
                #[cfg(feature = "offline-accounts")]
                FriendAction::NewRecovery => {
                    let code = t
                        .arctic_token(account.clone())
                        .and_then(|(base, token)| friends::new_recovery(&base, &token))
                        .map_err(|e| e.to_string());
                    t.send(Event::RecoveryCode(code));
                }
                // Before any sign-in: the name belongs to the old PC's key.
                FriendAction::Recover(code) => {
                    let base = cosmetics::base_url();
                    let result = cosmetics::recover_offline(t.dirs(), &base, &account, code)
                        .map(|()| "Your profile and friends are here now".to_owned())
                        .map_err(|e| e.to_string());
                    t.send(Event::FriendDone(result));
                }
                _ => {
                    let result = t
                        .friend_action_blocking(account.clone(), &action)
                        .map_err(|e| e.to_string());
                    t.send(Event::FriendDone(result));
                }
            }
            t.friends_refresh(account);
        });
    }

    /// What happened, in words for a toast (empty = nothing to say).
    fn friend_action_blocking(&self, account: Account, action: &FriendAction) -> Result<String> {
        let (base, token) = self.arctic_token(account)?;
        Ok(match action {
            #[cfg(feature = "offline-accounts")]
            FriendAction::NewRecovery => String::new(),
            FriendAction::Recover(_) => String::new(),
            FriendAction::Request(who) => {
                let r = friends::request(&base, &token, who)?;
                if r.friends {
                    format!("You and {} are friends now", r.name)
                } else {
                    format!("Asked {} to be friends", r.name)
                }
            }
            FriendAction::Accept(id) => {
                friends::accept(&base, &token, id)?;
                "Friend added".into()
            }
            FriendAction::Remove(id) => {
                friends::remove(&base, &token, id)?;
                String::new()
            }
            FriendAction::Invite(id, to) => {
                friends::invite(&base, &token, id, to)?;
                "Invite sent".into()
            }
            FriendAction::Dismiss(id) => {
                friends::dismiss(&base, &token, id)?;
                String::new()
            }
            FriendAction::Update(name, settings) => {
                friends::update(&base, &token, name.as_deref(), settings)?;
                "Saved".into()
            }
            FriendAction::Link(other) => {
                let (_, other_token) = self.arctic_token(other.clone())?;
                friends::link(&base, &token, &other_token)?;
                format!("Linked {}", other.username)
            }
            FriendAction::Unlink(uuid) => {
                friends::unlink(&base, &token, uuid)?;
                "Unlinked".into()
            }
        })
    }
}

/// Results the Friends page shows.
pub type FriendsView = (Profile, Overview);

impl Tasks {
    /// The conversation with a friend (and mark it read).
    pub fn chat_open(&self, account: Account, friend: String) {
        self.run(move |t| {
            let result = t
                .arctic_token(account)
                .and_then(|(base, token)| {
                    let list = friends::history(&base, &token, &friend, None)?;
                    friends::mark_read(&base, &token, &friend)?;
                    Ok(list)
                })
                .map_err(|e| e.to_string());
            t.send(Event::ChatHistory(friend, result));
        });
    }

    pub fn chat_send(&self, account: Account, friend: String, text: String) {
        self.run(move |t| {
            let result = t
                .arctic_token(account)
                .and_then(|(base, token)| friends::send(&base, &token, &friend, &text))
                .map_err(|e| e.to_string());
            t.send(Event::ChatSent(result));
        });
    }

    /// Messages newer than `after`; `read_from` is the open chat (marked read).
    pub fn chat_poll(&self, account: Account, after: i64, read_from: Option<String>) {
        self.run(move |t| {
            let result = t
                .arctic_token(account)
                .and_then(|(base, token)| {
                    let list = friends::new_messages(&base, &token, after)?;
                    if let Some(f) = read_from.filter(|f| list.iter().any(|m| &m.from == f)) {
                        friends::mark_read(&base, &token, &f)?;
                    }
                    Ok(list)
                })
                .map_err(|e| e.to_string());
            t.send(Event::ChatNew(after, result));
        });
    }
}

/// Widths tried when shrinking a screenshot to send (largest first).
const SEND_WIDTHS: [u32; 3] = [1280, 960, 720];

impl Tasks {
    /// Shrink a screenshot, upload it and send it to a friend.
    pub fn chat_send_image(&self, account: Account, friend: String, path: std::path::PathBuf) {
        self.run(move |t| {
            let result = (|| {
                let png = shrink_png(&path).map_err(arctic_core::Error::Other)?;
                let (base, token) = t.arctic_token(account)?;
                let id = friends::upload(&base, &token, &png)?;
                friends::send_with(&base, &token, &friend, "", Some(&id))
            })()
            .map_err(|e| e.to_string());
            t.send(Event::ChatSent(result));
        });
    }

    /// A screenshot from a chat, to show it.
    pub fn chat_image(&self, account: Account, id: String) {
        self.run(move |t| {
            let result = t
                .arctic_token(account)
                .and_then(|(base, token)| friends::image(&base, &token, &id))
                .map_err(|e| e.to_string());
            t.send(Event::ChatImage(id, result));
        });
    }
}

/// The screenshot as a PNG small enough to send.
fn shrink_png(path: &std::path::Path) -> std::result::Result<Vec<u8>, String> {
    let img = image::open(path).map_err(|e| format!("couldn't read that screenshot: {e}"))?;
    for width in SEND_WIDTHS {
        let small = if img.width() > width {
            img.resize(width, u32::MAX, image::imageops::FilterType::Triangle)
        } else {
            img.clone()
        };
        let mut out = std::io::Cursor::new(Vec::new());
        small
            .write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let bytes = out.into_inner();
        if bytes.len() <= friends::MAX_UPLOAD {
            return Ok(bytes);
        }
    }
    Err("that screenshot is too big to send".into())
}
