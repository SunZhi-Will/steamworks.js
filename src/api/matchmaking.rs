use napi_derive::napi;

#[napi]
pub mod matchmaking {
    use crate::api::localplayer::PlayerSteamId;
    use napi::bindgen_prelude::{BigInt, Error};
    use std::collections::HashMap;
    use steamworks::LobbyId;
    use tokio::sync::oneshot;

    #[napi]
    pub enum LobbyType {
        Private,
        FriendsOnly,
        Public,
        Invisible,
    }

    #[napi]
    pub struct Lobby {
        pub id: BigInt,
        lobby_id: LobbyId,
    }

    #[napi]
    impl Lobby {
        #[napi]
        pub async fn join(&self) -> Result<Lobby, Error> {
            join_lobby(self.id.clone()).await
        }

        #[napi]
        pub fn leave(&self) {
            let client = crate::client::get_client();
            client.matchmaking().leave_lobby(self.lobby_id);
        }

        #[napi]
        pub fn open_invite_dialog(&self) {
            let client = crate::client::get_client();
            client.friends().activate_invite_dialog(self.lobby_id);
        }

        #[napi]
        pub fn get_member_count(&self) -> usize {
            let client = crate::client::get_client();
            client.matchmaking().lobby_member_count(self.lobby_id)
        }

        #[napi]
        pub fn get_member_limit(&self) -> Option<usize> {
            let client = crate::client::get_client();
            client.matchmaking().lobby_member_limit(self.lobby_id)
        }

        #[napi]
        pub fn get_members(&self) -> Vec<PlayerSteamId> {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .lobby_members(self.lobby_id)
                .into_iter()
                .map(PlayerSteamId::from_steamid)
                .collect()
        }

        #[napi]
        pub fn get_owner(&self) -> PlayerSteamId {
            let client = crate::client::get_client();
            PlayerSteamId::from_steamid(client.matchmaking().lobby_owner(self.lobby_id))
        }

        #[napi]
        pub fn set_joinable(&self, joinable: bool) -> bool {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .set_lobby_joinable(self.lobby_id, joinable)
        }

        #[napi]
        pub fn get_data(&self, key: String) -> Option<String> {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .lobby_data(self.lobby_id, &key)
                .map(|s| s.to_string())
        }

        #[napi]
        pub fn set_data(&self, key: String, value: String) -> bool {
            let client = crate::client::get_client();
            client
                .matchmaking()
                .set_lobby_data(self.lobby_id, &key, &value)
        }

        #[napi]
        pub fn delete_data(&self, key: String) -> bool {
            let client = crate::client::get_client();
            client.matchmaking().delete_lobby_data(self.lobby_id, &key)
        }

        /// Get an object containing all the lobby data
        #[napi]
        pub fn get_full_data(&self) -> HashMap<String, String> {
            let client = crate::client::get_client();

            let mut data = HashMap::new();

            let count = client.matchmaking().lobby_data_count(self.lobby_id);
            for i in 0..count {
                let maybe_lobby_data = client.matchmaking().lobby_data_by_index(self.lobby_id, i);

                if let Some((key, value)) = maybe_lobby_data {
                    data.insert(key, value);
                }
            }

            data
        }

        /// Merge current lobby data with provided data in a single batch
        /// @returns true if all data was set successfully
        #[napi]
        pub fn merge_full_data(&self, data: HashMap<String, String>) -> bool {
            let matchmaking = crate::client::get_client().matchmaking();
            data.iter()
                .map(|(key, value)| matchmaking.set_lobby_data(self.lobby_id, key, value))
                .all(|x| x)
        }
    }

    #[napi]
    pub async fn create_lobby(lobby_type: LobbyType, max_members: u32) -> Result<Lobby, Error> {
        let client = crate::client::get_client();

        let (tx, rx) = oneshot::channel();

        client.matchmaking().create_lobby(
            match lobby_type {
                LobbyType::Private => steamworks::LobbyType::Private,
                LobbyType::FriendsOnly => steamworks::LobbyType::FriendsOnly,
                LobbyType::Public => steamworks::LobbyType::Public,
                LobbyType::Invisible => steamworks::LobbyType::Invisible,
            },
            max_members,
            |result| {
                tx.send(result).unwrap();
            },
        );

        rx.await
            .unwrap()
            .map(|lobby_id| Lobby {
                id: BigInt::from(lobby_id.raw()),
                lobby_id,
            })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    pub async fn join_lobby(lobby_id: BigInt) -> Result<Lobby, Error> {
        let client = crate::client::get_client();

        let (tx, rx) = oneshot::channel();

        client.matchmaking().join_lobby(
            steamworks::LobbyId::from_raw(lobby_id.get_u64().1),
            |result| {
                tx.send(result).unwrap();
            },
        );

        rx.await
            .unwrap()
            .map(|lobby_id| Lobby {
                id: BigInt::from(lobby_id.raw()),
                lobby_id,
            })
            .map_err(|_| Error::from_reason("Failed to join lobby".to_string()))
    }

    #[napi]
    pub async fn get_lobbies() -> Result<Vec<Lobby>, Error> {
        let client = crate::client::get_client();

        let (tx, rx) = oneshot::channel();

        client.matchmaking().request_lobby_list(|lobbies| {
            tx.send(lobbies).unwrap();
        });

        rx.await
            .unwrap()
            .map(|lobbies| {
                lobbies
                    .iter()
                    .map(|lobby_id| Lobby {
                        id: BigInt::from(lobby_id.raw()),
                        lobby_id: *lobby_id,
                    })
                    .collect()
            })
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    #[napi]
    /// ELobbyComparison for lobby-list filters.
    pub enum LobbyComparison {
        EqualToOrLessThan = -2,
        LessThan = -1,
        Equal = 0,
        GreaterThan = 1,
        EqualToOrGreaterThan = 2,
        NotEqual = 3,
    }

    #[napi]
    /// ELobbyDistanceFilter. Use Worldwide to find a friend's room by code.
    pub enum LobbyDistanceFilter {
        Close = 0,
        Default = 1,
        Far = 2,
        Worldwide = 3,
    }

    #[napi(object)]
    pub struct LobbyStringFilter {
        pub key: String,
        pub value: String,
        pub comparison: i32,
    }

    #[napi(object)]
    pub struct LobbyNumberFilter {
        pub key: String,
        pub value: i32,
        pub comparison: i32,
    }

    #[napi(object)]
    pub struct LobbyListFilter {
        pub strings: Option<Vec<LobbyStringFilter>>,
        pub numbers: Option<Vec<LobbyNumberFilter>>,
        pub distance: Option<i32>,
        pub result_count: Option<i32>,
        pub slots_available: Option<i32>,
    }

    fn mm_iface() -> *mut steamworks::sys::ISteamMatchmaking {
        let _ = crate::client::get_client();
        unsafe { steamworks::sys::SteamAPI_SteamMatchmaking_v009() }
    }

    fn comparison(v: i32) -> steamworks::sys::ELobbyComparison {
        use steamworks::sys::ELobbyComparison as C;
        match v {
            -2 => C::k_ELobbyComparisonEqualToOrLessThan,
            -1 => C::k_ELobbyComparisonLessThan,
            1 => C::k_ELobbyComparisonGreaterThan,
            2 => C::k_ELobbyComparisonEqualToOrGreaterThan,
            3 => C::k_ELobbyComparisonNotEqual,
            _ => C::k_ELobbyComparisonEqual,
        }
    }

    fn distance(v: i32) -> steamworks::sys::ELobbyDistanceFilter {
        use steamworks::sys::ELobbyDistanceFilter as D;
        match v {
            0 => D::k_ELobbyDistanceFilterClose,
            2 => D::k_ELobbyDistanceFilterFar,
            3 => D::k_ELobbyDistanceFilterWorldwide,
            _ => D::k_ELobbyDistanceFilterDefault,
        }
    }

    /// RequestLobbyList with filters applied immediately before the call
    /// (Steam clears them after every request). Resolves to lobby ids; their
    /// data is readable right away with getLobbyDataById. Only Public and
    /// Invisible, joinable, not-full lobbies are ever returned.
    #[napi]
    pub async fn request_lobby_list_filtered(filter: LobbyListFilter) -> Result<Vec<BigInt>, Error> {
        let client = crate::client::get_client();
        let mm = mm_iface();
        if !mm.is_null() {
            unsafe {
                for f in filter.strings.unwrap_or_default() {
                    let (Ok(k), Ok(v)) = (std::ffi::CString::new(f.key), std::ffi::CString::new(f.value)) else { continue };
                    steamworks::sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListStringFilter(mm, k.as_ptr(), v.as_ptr(), comparison(f.comparison));
                }
                for f in filter.numbers.unwrap_or_default() {
                    let Ok(k) = std::ffi::CString::new(f.key) else { continue };
                    steamworks::sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListNumericalFilter(mm, k.as_ptr(), f.value, comparison(f.comparison));
                }
                if let Some(d) = filter.distance {
                    steamworks::sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListDistanceFilter(mm, distance(d));
                }
                if let Some(n) = filter.result_count {
                    steamworks::sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListResultCountFilter(mm, n);
                }
                if let Some(n) = filter.slots_available {
                    steamworks::sys::SteamAPI_ISteamMatchmaking_AddRequestLobbyListFilterSlotsAvailable(mm, n);
                }
            }
        }

        let (tx, rx) = oneshot::channel();
        client.matchmaking().request_lobby_list(|lobbies| {
            let _ = tx.send(lobbies);
        });
        rx.await
            .map_err(|_| Error::from_reason("lobby list request dropped".to_string()))?
            .map(|lobbies| lobbies.iter().map(|l| BigInt::from(l.raw())).collect())
            .map_err(|e| Error::from_reason(e.to_string()))
    }

    /// Lobby data for any lobby we have data for (search results, lobbies we
    /// are in, or after requestLobbyData). '' when unknown.
    #[napi]
    pub fn get_lobby_data_by_id(lobby_id: BigInt, key: String) -> String {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .lobby_data(LobbyId::from_raw(lobby_id.get_u64().1), &key)
            .map(|s| s.to_string())
            .unwrap_or_default()
    }

    #[napi]
    pub fn get_lobby_member_count(lobby_id: BigInt) -> u32 {
        let client = crate::client::get_client();
        client.matchmaking().lobby_member_count(LobbyId::from_raw(lobby_id.get_u64().1)) as u32
    }

    #[napi]
    pub fn get_lobby_member_limit(lobby_id: BigInt) -> u32 {
        let client = crate::client::get_client();
        client
            .matchmaking()
            .lobby_member_limit(LobbyId::from_raw(lobby_id.get_u64().1))
            .unwrap_or(0) as u32
    }

    /// Ask Steam for a lobby's data without joining it (a friend's room).
    /// LobbyDataUpdate fires when it arrives.
    #[napi]
    pub fn request_lobby_data(lobby_id: BigInt) -> bool {
        let mm = mm_iface();
        if mm.is_null() {
            return false;
        }
        unsafe { steamworks::sys::SteamAPI_ISteamMatchmaking_RequestLobbyData(mm, lobby_id.get_u64().1) }
    }

    /// Invite a user to a lobby without the overlay (the in-game invite list).
    #[napi]
    pub fn invite_user_to_lobby(lobby_id: BigInt, steam_id64: BigInt) -> bool {
        let mm = mm_iface();
        if mm.is_null() {
            return false;
        }
        unsafe {
            steamworks::sys::SteamAPI_ISteamMatchmaking_InviteUserToLobby(mm, lobby_id.get_u64().1, steam_id64.get_u64().1)
        }
    }
}
