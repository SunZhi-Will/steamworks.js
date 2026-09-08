use napi_derive::napi;

#[napi]
pub mod networking {
    use napi::{
        bindgen_prelude::{BigInt, Buffer},
        Error,
    };
    use steamworks::SteamId;

    use crate::api::localplayer::PlayerSteamId;

    #[napi(object)]
    pub struct P2PPacket {
        pub data: Buffer,
        pub size: i32,
        pub steam_id: PlayerSteamId,
    }

    #[napi]
    /// The method used to send a packet
    pub enum SendType {
        /// Send the packet directly over udp.
        ///
        /// Can't be larger than 1200 bytes
        Unreliable,
        /// Like `Unreliable` but doesn't buffer packets
        /// sent before the connection has started.
        UnreliableNoDelay,
        /// Reliable packet sending.
        ///
        /// Can't be larger than 1 megabyte.
        Reliable,
        /// Like `Reliable` but applies the nagle
        /// algorithm to packets being sent
        ReliableWithBuffering,
    }

    #[napi]
    pub fn send_p2p_packet(
        steam_id64: BigInt,
        send_type: SendType,
        data: Buffer,
    ) -> Result<bool, Error> {
        let client = crate::client::get_client();
        let result = client.networking().send_p2p_packet(
            SteamId::from_raw(steam_id64.get_u64().1),
            match send_type {
                SendType::Unreliable => steamworks::SendType::Unreliable,
                SendType::UnreliableNoDelay => steamworks::SendType::UnreliableNoDelay,
                SendType::Reliable => steamworks::SendType::Reliable,
                SendType::ReliableWithBuffering => steamworks::SendType::ReliableWithBuffering,
            },
            &data,
        );
        Ok(result)
    }

    #[napi]
    pub fn is_p2p_packet_available() -> i32 {
        let client = crate::client::get_client();
        client
            .networking()
            .is_p2p_packet_available()
            .unwrap_or_default() as i32
    }

    #[napi]
    pub fn read_p2p_packet(size: i32) -> Result<P2PPacket, Error> {
        let client = crate::client::get_client();
        let mut buffer = vec![0; size as usize];

        client
            .networking()
            .read_p2p_packet(&mut buffer)
            .map(|(steam_id, read_size)| P2PPacket {
                data: buffer.into(),
                size: read_size as i32,
                steam_id: PlayerSteamId::from_steamid(steam_id),
            })
            .ok_or_else(|| {
                Error::new(
                    napi::Status::GenericFailure,
                    "No packet available".to_string(),
                )
            })
    }

    #[napi]
    pub fn accept_p2p_session(steam_id64: BigInt) {
        let client = crate::client::get_client();
        client
            .networking()
            .accept_p2p_session(SteamId::from_raw(steam_id64.get_u64().1));
    }

    #[napi(object)]
    pub struct P2PSessionState {
        pub connection_active: bool,
        pub connecting: bool,
        pub using_relay: bool,
        pub session_error: u32,
        pub bytes_queued: i32,
        pub packets_queued: i32,
    }

    /// Gets the connection state to the specified user.
    ///
    /// Returns `null` if there is no active P2P session with the given user
    /// (mirrors `ISteamNetworking::GetP2PSessionState` returning `false`).
    #[napi]
    pub fn get_p2p_session_state(steam_id64: BigInt) -> Option<P2PSessionState> {
        let client = crate::client::get_client();
        client
            .networking()
            .get_p2p_session_state(SteamId::from_raw(steam_id64.get_u64().1))
            .map(|state| P2PSessionState {
                connection_active: state.connection_active,
                connecting: state.connecting,
                using_relay: state.using_relay,
                session_error: u8::from(state.error) as u32,
                bytes_queued: state.bytes_queued_for_send,
                packets_queued: state.packets_queued_for_send,
            })
    }

    /// Closes the p2p connection to the given user, freeing up resources under the hood.
    #[napi]
    pub fn close_p2p_session(steam_id64: BigInt) -> bool {
        let client = crate::client::get_client();
        client
            .networking()
            .close_p2p_session(SteamId::from_raw(steam_id64.get_u64().1))
    }
}
