use napi_derive::napi;

/// ISteamNetworkingMessages: the connectionless, UDP-like P2P API that
/// replaces the deprecated ISteamNetworking. Traffic goes over Valve's relay
/// network (SDR) by default. Talks to the flat C API directly so that the
/// session-request callback never auto-rejects: the JS side decides whether
/// to accept (see `acceptSessionWithUser`) after checking lobby membership.
#[napi]
#[allow(non_snake_case)]
pub mod networkingMessages {
    use napi::bindgen_prelude::{BigInt, Buffer};
    use steamworks::sys;

    #[napi]
    /// k_nSteamNetworkingSend_* flags for `sendMessageToUser`.
    pub enum SendFlags {
        Unreliable = 0,
        NoNagle = 1,
        NoDelay = 4,
        UnreliableNoDelay = 5,
        Reliable = 8,
        ReliableNoNagle = 9,
        AutoRestartBrokenSession = 32,
    }

    #[napi(object)]
    pub struct NetworkingMessage {
        pub steam_id64: BigInt,
        pub data: Buffer,
        pub channel: i32,
    }

    #[napi(object)]
    pub struct SessionConnectionInfo {
        pub state: i32,
        pub end_reason: i32,
        pub end_debug: String,
        pub relayed: bool,
        pub ping: i32,
        pub quality_local: f64,
        pub quality_remote: f64,
        pub out_bytes_per_sec: f64,
        pub in_bytes_per_sec: f64,
        pub pending_unreliable: i32,
        pub pending_reliable: i32,
        pub sent_unacked_reliable: i32,
        pub queue_time_usec: f64,
    }

    fn iface() -> *mut sys::ISteamNetworkingMessages {
        // Make sure the client (and so SteamAPI_Init) exists first.
        let _ = crate::client::get_client();
        unsafe { sys::SteamAPI_SteamNetworkingMessages_SteamAPI_v002() }
    }

    pub(crate) fn identity(steam_id64: u64) -> sys::SteamNetworkingIdentity {
        let mut id: sys::SteamNetworkingIdentity = unsafe { std::mem::zeroed() };
        unsafe { sys::SteamAPI_SteamNetworkingIdentity_SetSteamID64(&mut id, steam_id64) };
        id
    }

    pub(crate) fn identity_steam_id64(id: &sys::SteamNetworkingIdentity) -> u64 {
        let mut copy = *id;
        unsafe { sys::SteamAPI_SteamNetworkingIdentity_GetSteamID64(&mut copy) }
    }

    /// Returns the raw EResult (1 = OK).
    #[napi]
    pub fn send_message_to_user(steam_id64: BigInt, data: Buffer, flags: i32, channel: i32) -> i32 {
        let net = iface();
        if net.is_null() {
            return 2; // k_EResultFail
        }
        let id = identity(steam_id64.get_u64().1);
        let bytes: &[u8] = &data;
        let result = unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_SendMessageToUser(
                net,
                &id,
                bytes.as_ptr() as *const std::os::raw::c_void,
                bytes.len() as u32,
                flags,
                channel,
            )
        };
        result as i32
    }

    /// Pulls up to `maxMessages` from one channel. Every message is copied
    /// into a Buffer and released before returning.
    #[napi]
    pub fn receive_messages_on_channel(channel: i32, max_messages: u32) -> Vec<NetworkingMessage> {
        let net = iface();
        if net.is_null() || max_messages == 0 {
            return Vec::new();
        }
        let cap = max_messages.min(1024) as usize;
        let mut ptrs: Vec<*mut sys::SteamNetworkingMessage_t> = vec![std::ptr::null_mut(); cap];
        let count = unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel(
                net,
                channel,
                ptrs.as_mut_ptr(),
                cap as i32,
            )
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
                let sender = identity_steam_id64(&msg.m_identityPeer);
                out.push(NetworkingMessage {
                    steam_id64: BigInt::from(sender),
                    data: data.into(),
                    channel,
                });
                sys::SteamAPI_SteamNetworkingMessage_t_Release(ptr);
            }
        }
        out
    }

    #[napi]
    pub fn accept_session_with_user(steam_id64: BigInt) -> bool {
        let net = iface();
        if net.is_null() {
            return false;
        }
        let id = identity(steam_id64.get_u64().1);
        unsafe { sys::SteamAPI_ISteamNetworkingMessages_AcceptSessionWithUser(net, &id) }
    }

    #[napi]
    pub fn close_session_with_user(steam_id64: BigInt) -> bool {
        let net = iface();
        if net.is_null() {
            return false;
        }
        let id = identity(steam_id64.get_u64().1);
        unsafe { sys::SteamAPI_ISteamNetworkingMessages_CloseSessionWithUser(net, &id) }
    }

    #[napi]
    pub fn close_channel_with_user(steam_id64: BigInt, channel: i32) -> bool {
        let net = iface();
        if net.is_null() {
            return false;
        }
        let id = identity(steam_id64.get_u64().1);
        unsafe { sys::SteamAPI_ISteamNetworkingMessages_CloseChannelWithUser(net, &id, channel) }
    }

    /// null when there is no session with that user.
    #[napi]
    pub fn get_session_connection_info(steam_id64: BigInt) -> Option<SessionConnectionInfo> {
        let net = iface();
        if net.is_null() {
            return None;
        }
        let id = identity(steam_id64.get_u64().1);
        let mut info: sys::SteamNetConnectionInfo_t = unsafe { std::mem::zeroed() };
        let mut status: sys::SteamNetConnectionRealTimeStatus_t = unsafe { std::mem::zeroed() };
        let state = unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_GetSessionConnectionInfo(net, &id, &mut info, &mut status)
        };
        let state = state as i32;
        if state == 0 {
            return None;
        }
        let end_debug = unsafe {
            std::ffi::CStr::from_ptr(info.m_szEndDebug.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        let relayed = (info.m_nFlags & sys::k_nSteamNetworkConnectionInfoFlags_Relayed as i32) != 0;
        Some(SessionConnectionInfo {
            state,
            end_reason: info.m_eEndReason,
            end_debug,
            relayed,
            ping: status.m_nPing,
            quality_local: status.m_flConnectionQualityLocal as f64,
            quality_remote: status.m_flConnectionQualityRemote as f64,
            out_bytes_per_sec: status.m_flOutBytesPerSec as f64,
            in_bytes_per_sec: status.m_flInBytesPerSec as f64,
            pending_unreliable: status.m_cbPendingUnreliable,
            pending_reliable: status.m_cbPendingReliable,
            sent_unacked_reliable: status.m_cbSentUnackedReliable,
            queue_time_usec: status.m_usecQueueTime as f64,
        })
    }
}

/// Callback payloads for the SteamCallback enum (see callback.rs). Declared
/// here rather than reusing steamworks-rs's own structs because those keep
/// their fields private and are not Serialize.
pub struct MessagesSessionRequest {
    pub remote: u64,
}

unsafe impl steamworks::Callback for MessagesSessionRequest {
    const ID: i32 = steamworks::sys::SteamNetworkingMessagesSessionRequest_t_k_iCallback as i32;
    unsafe fn from_raw(raw: *mut std::os::raw::c_void) -> Self {
        let cb = raw
            .cast::<steamworks::sys::SteamNetworkingMessagesSessionRequest_t>()
            .read_unaligned();
        Self {
            remote: networkingMessages::identity_steam_id64(&cb.m_identityRemote),
        }
    }
}

pub struct MessagesSessionFailed {
    pub remote: u64,
    pub end_reason: i32,
    pub state: i32,
}

unsafe impl steamworks::Callback for MessagesSessionFailed {
    const ID: i32 = steamworks::sys::SteamNetworkingMessagesSessionFailed_t_k_iCallback as i32;
    unsafe fn from_raw(raw: *mut std::os::raw::c_void) -> Self {
        let cb = raw
            .cast::<steamworks::sys::SteamNetworkingMessagesSessionFailed_t>()
            .read_unaligned();
        Self {
            remote: networkingMessages::identity_steam_id64(&cb.m_info.m_identityRemote),
            end_reason: cb.m_info.m_eEndReason,
            state: cb.m_info.m_eState as i32,
        }
    }
}
