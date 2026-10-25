use napi_derive::napi;

/// ISteamGameServer + the game-server flavour of ISteamNetworkingMessages.
///
/// A dedicated server logs on to Steam anonymously with its own (game server)
/// Steam ID instead of a user account, so it can run on the same machine as the
/// player's own game without sharing that user's Steam identity (two processes
/// on one user account get each other's callbacks and P2P messages).
///
/// Callbacks are not pushed to JS: the server keeps small queues that JS drains
/// on its own tick (`takeSessionRequests`, `takeSessionFailures`, `takeEvents`),
/// so nothing here needs a threadsafe function. `runCallbacks` must be called
/// regularly (the desktop shell does it on the same timer that drains messages).
#[napi]
#[allow(non_snake_case)]
pub mod gameServer {
    use napi::bindgen_prelude::{BigInt, Buffer, Error};
    use std::net::Ipv4Addr;
    use std::sync::Mutex;
    use steamworks::{sys, CallbackHandle, Server, ServerMode};

    use crate::api::networking_messages::networkingMessages::NetworkingMessage;
    use crate::api::networking_messages::{MessagesSessionFailed, MessagesSessionRequest};

    struct State {
        server: Server,
        _handles: Vec<CallbackHandle>,
    }
    unsafe impl Send for State {}

    lazy_static! {
        static ref STATE: Mutex<Option<State>> = Mutex::new(None);
        static ref SESSION_REQUESTS: Mutex<Vec<u64>> = Mutex::new(Vec::new());
        static ref SESSION_FAILURES: Mutex<Vec<SessionFailure>> = Mutex::new(Vec::new());
        static ref EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    }

    #[napi(object)]
    #[derive(Clone)]
    pub struct SessionFailure {
        pub steam_id64: BigInt,
        pub end_reason: i32,
        pub state: i32,
    }

    fn with_server<T>(f: impl FnOnce(&Server) -> T) -> Option<T> {
        STATE.lock().unwrap().as_ref().map(|s| f(&s.server))
    }

    fn messages_iface() -> *mut sys::ISteamNetworkingMessages {
        if STATE.lock().unwrap().is_none() {
            return std::ptr::null_mut();
        }
        unsafe { sys::SteamAPI_SteamGameServerNetworkingMessages_SteamAPI_v002() }
    }

    /// Starts the game server. `appId` is the GAME's AppID (Steam runs a
    /// dedicated server under its base game). Ports: 0 lets Steam pick the
    /// game port; the query port can share the game socket (QUERY_PORT_SHARED)
    /// because this server never answers server-browser pings itself.
    /// Call `logOnAnonymous` next; `isLoggedOn` turns true once Steam answers.
    #[napi]
    pub fn init(app_id: u32, game_port: u16, version: String) -> Result<(), Error> {
        if STATE.lock().unwrap().is_some() {
            return Ok(());
        }
        // SteamInternal_GameServer_Init reads the AppID from the environment
        // when there is no steam_appid.txt next to the executable.
        std::env::set_var("SteamAppId", app_id.to_string());
        std::env::set_var("SteamGameId", app_id.to_string());
        let (server, _client) = Server::init(
            Ipv4Addr::UNSPECIFIED,
            game_port,
            steamworks::QUERY_PORT_SHARED,
            ServerMode::NoAuthentication,
            &version,
        )
        .map_err(|e| Error::from_reason(format!("{:?}", e)))?;

        let mut handles = Vec::new();
        handles.push(server.register_callback(move |v: MessagesSessionRequest| {
            SESSION_REQUESTS.lock().unwrap().push(v.remote);
        }));
        handles.push(server.register_callback(move |v: MessagesSessionFailed| {
            SESSION_FAILURES.lock().unwrap().push(SessionFailure {
                steam_id64: BigInt::from(v.remote),
                end_reason: v.end_reason,
                state: v.state,
            });
        }));
        handles.push(server.register_callback(move |_: steamworks::SteamServersConnected| {
            EVENTS.lock().unwrap().push("connected".into());
        }));
        handles.push(server.register_callback(move |_: steamworks::SteamServersDisconnected| {
            EVENTS.lock().unwrap().push("disconnected".into());
        }));
        handles.push(server.register_callback(move |_: steamworks::SteamServerConnectFailure| {
            EVENTS.lock().unwrap().push("connectFailure".into());
        }));
        *STATE.lock().unwrap() = Some(State { server, _handles: handles });
        Ok(())
    }

    #[napi]
    pub fn is_initialized() -> bool {
        STATE.lock().unwrap().is_some()
    }

    /// Server identity and browser fields. Safe to call before or after logon.
    #[napi]
    pub fn configure(product: String, description: String, server_name: String, max_players: i32, password_protected: bool) {
        with_server(|s| {
            s.set_product(&product);
            s.set_game_description(&description);
            s.set_dedicated_server(true);
            s.set_server_name(&server_name);
            s.set_max_players(max_players);
            s.set_password_protected(password_protected);
        });
    }

    #[napi]
    pub fn set_game_tags(tags: String) {
        with_server(|s| s.set_game_tags(&tags));
    }

    #[napi]
    pub fn set_key_value(key: String, value: String) {
        with_server(|s| s.set_key_value(&key, &value));
    }

    #[napi]
    pub fn set_advertise_server_active(active: bool) {
        with_server(|s| s.set_advertise_server_active(active));
    }

    #[napi]
    pub fn log_on_anonymous() {
        with_server(|s| s.log_on_anonymous());
    }

    #[napi]
    pub fn is_logged_on() -> bool {
        with_server(|_s| unsafe { sys::SteamAPI_ISteamGameServer_BLoggedOn(s_raw()) }).unwrap_or(false)
    }

    fn s_raw() -> *mut sys::ISteamGameServer {
        unsafe { sys::SteamAPI_SteamGameServer_v015() }
    }

    /// The server's own Steam ID (valid once logged on; 0 before).
    #[napi]
    pub fn steam_id64() -> BigInt {
        BigInt::from(with_server(|s| s.steam_id().raw()).unwrap_or(0))
    }

    #[napi]
    pub fn run_callbacks() {
        with_server(|s| s.run_callbacks());
    }

    /// Lets P2P sessions to this server go over Valve's relay network.
    #[napi]
    pub fn init_relay_network_access() {
        if STATE.lock().unwrap().is_none() {
            return;
        }
        // The game-server context has no exported SteamNetworkingUtils accessor in
        // this SDK binding; ISteamNetworkingMessages starts relay access on its
        // own the first time a session needs it, so this is only a hint.
        unsafe {
            let utils = sys::SteamAPI_SteamNetworkingUtils_SteamAPI_v004();
            if !utils.is_null() && crate::client::has_client() {
                sys::SteamAPI_ISteamNetworkingUtils_InitRelayNetworkAccess(utils);
            }
        }
    }

    #[napi]
    pub fn take_session_requests() -> Vec<BigInt> {
        SESSION_REQUESTS.lock().unwrap().drain(..).map(BigInt::from).collect()
    }

    #[napi]
    pub fn take_session_failures() -> Vec<SessionFailure> {
        SESSION_FAILURES.lock().unwrap().drain(..).collect()
    }

    /// 'connected' / 'disconnected' / 'connectFailure', oldest first.
    #[napi]
    pub fn take_events() -> Vec<String> {
        EVENTS.lock().unwrap().drain(..).collect()
    }

    fn identity(steam_id64: u64) -> sys::SteamNetworkingIdentity {
        crate::api::networking_messages::networkingMessages::identity(steam_id64)
    }

    /// Returns the raw EResult (1 = OK).
    #[napi]
    pub fn send_message_to_user(steam_id64: BigInt, data: Buffer, flags: i32, channel: i32) -> i32 {
        let net = messages_iface();
        if net.is_null() {
            return 2;
        }
        let id = identity(steam_id64.get_u64().1);
        let bytes: &[u8] = &data;
        unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_SendMessageToUser(
                net,
                &id,
                bytes.as_ptr() as *const std::os::raw::c_void,
                bytes.len() as u32,
                flags,
                channel,
            ) as i32
        }
    }

    #[napi]
    pub fn receive_messages_on_channel(channel: i32, max_messages: u32) -> Vec<NetworkingMessage> {
        let net = messages_iface();
        if net.is_null() || max_messages == 0 {
            return Vec::new();
        }
        let cap = max_messages.min(1024) as usize;
        let mut ptrs: Vec<*mut sys::SteamNetworkingMessage_t> = vec![std::ptr::null_mut(); cap];
        let count = unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel(net, channel, ptrs.as_mut_ptr(), cap as i32)
        };
        let mut out = Vec::with_capacity(count.max(0) as usize);
        for ptr in ptrs.into_iter().take(count.max(0) as usize) {
            if ptr.is_null() {
                continue;
            }
            unsafe {
                let msg = &*ptr;
                let len = msg.m_cbSize.max(0) as usize;
                let data = if len == 0 || msg.m_pData.is_null() {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(msg.m_pData as *const u8, len).to_vec()
                };
                let sender = crate::api::networking_messages::networkingMessages::identity_steam_id64(&msg.m_identityPeer);
                out.push(NetworkingMessage { steam_id64: BigInt::from(sender), data: data.into(), channel });
                sys::SteamAPI_SteamNetworkingMessage_t_Release(ptr);
            }
        }
        out
    }

    #[napi]
    pub fn accept_session_with_user(steam_id64: BigInt) -> bool {
        let net = messages_iface();
        if net.is_null() {
            return false;
        }
        let id = identity(steam_id64.get_u64().1);
        unsafe { sys::SteamAPI_ISteamNetworkingMessages_AcceptSessionWithUser(net, &id) }
    }

    #[napi]
    pub fn close_session_with_user(steam_id64: BigInt) -> bool {
        let net = messages_iface();
        if net.is_null() {
            return false;
        }
        let id = identity(steam_id64.get_u64().1);
        unsafe { sys::SteamAPI_ISteamNetworkingMessages_CloseSessionWithUser(net, &id) }
    }

    /// Logs off and shuts the game server API down.
    #[napi]
    pub fn shutdown() {
        let mut st = STATE.lock().unwrap();
        if st.is_none() {
            return;
        }
        unsafe {
            sys::SteamAPI_ISteamGameServer_LogOff(s_raw());
        }
        *st = None;
        unsafe { sys::SteamGameServer_Shutdown() };
    }
}
