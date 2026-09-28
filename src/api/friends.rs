use napi_derive::napi;

/// ISteamFriends: the friends list, where friends are playing, and the
/// relationship check the game uses to let friends into a room.
#[napi]
pub mod friends {
    use napi::bindgen_prelude::BigInt;
    use steamworks::{sys, FriendFlags as RsFriendFlags, SteamId};

    #[napi]
    /// EFriendFlags, combine with bitwise OR.
    pub enum FriendFlags {
        None = 0,
        Blocked = 1,
        FriendshipRequested = 2,
        Immediate = 4,
        ClanMember = 8,
        OnGameServer = 16,
        RequestingFriendship = 128,
        RequestingInfo = 256,
        Ignored = 512,
        IgnoredFriend = 1024,
        ChatMember = 4096,
        All = 65535,
    }

    #[napi]
    /// EPersonaState.
    pub enum PersonaState {
        Offline = 0,
        Online = 1,
        Busy = 2,
        Away = 3,
        Snooze = 4,
        LookingToTrade = 5,
        LookingToPlay = 6,
        Invisible = 7,
    }

    #[napi(object)]
    pub struct FriendInfo {
        pub steam_id64: BigInt,
        pub name: String,
        pub state: i32,
        /// App the friend is running, null when not in a game.
        pub game_app_id: Option<u32>,
        /// Lobby the friend is in for that game, null when none.
        pub game_lobby_id: Option<BigInt>,
    }

    fn iface() -> *mut sys::ISteamFriends {
        let _ = crate::client::get_client();
        unsafe { sys::SteamAPI_SteamFriends_v018() }
    }

    #[napi]
    pub fn get_friends(flags: i32) -> Vec<FriendInfo> {
        let client = crate::client::get_client();
        let f = iface();
        let list = client
            .friends()
            .get_friends(RsFriendFlags::from_bits_truncate(flags as u16));
        list.into_iter()
            .map(|friend| {
                let id = friend.id();
                let state = if f.is_null() {
                    0
                } else {
                    unsafe { sys::SteamAPI_ISteamFriends_GetFriendPersonaState(f, id.raw()) as i32 }
                };
                let (game_app_id, game_lobby_id) = match friend.game_played() {
                    Some(g) => {
                        let lobby = g.lobby.raw();
                        (
                            Some(g.game.app_id().0),
                            if lobby == 0 { None } else { Some(BigInt::from(lobby)) },
                        )
                    }
                    None => (None, None),
                };
                FriendInfo {
                    steam_id64: BigInt::from(id.raw()),
                    name: friend.name(),
                    state,
                    game_app_id,
                    game_lobby_id,
                }
            })
            .collect()
    }

    /// Raw EFriendRelationship (3 = Friend).
    #[napi]
    pub fn get_friend_relationship(steam_id64: BigInt) -> i32 {
        let f = iface();
        if f.is_null() {
            return 0;
        }
        let id = SteamId::from_raw(steam_id64.get_u64().1);
        unsafe { sys::SteamAPI_ISteamFriends_GetFriendRelationship(f, id.raw()) as i32 }
    }
}
