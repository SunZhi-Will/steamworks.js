use napi_derive::napi;

/// ISteamNetworkingUtils: relay (SDR) warm-up and ping-location estimates.
#[napi]
#[allow(non_snake_case)]
pub mod networkingUtils {
    use steamworks::sys;

    fn iface() -> *mut sys::ISteamNetworkingUtils {
        let _ = crate::client::get_client();
        unsafe { sys::SteamAPI_SteamNetworkingUtils_SteamAPI_v004() }
    }

    /// Start measuring relay pings now, so the first P2P connection does not
    /// pay for it.
    #[napi]
    pub fn init_relay_network_access() {
        let u = iface();
        if !u.is_null() {
            unsafe { sys::SteamAPI_ISteamNetworkingUtils_InitRelayNetworkAccess(u) };
        }
    }

    /// Raw ESteamNetworkingAvailability (100 = Current).
    #[napi]
    pub fn get_relay_network_status() -> i32 {
        let u = iface();
        if u.is_null() {
            return -1;
        }
        unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_GetRelayNetworkStatus(u, std::ptr::null_mut()) as i32
        }
    }

    /// This machine's ping location as a string (safe to put in lobby data),
    /// or null until relay pings have been measured.
    #[napi]
    pub fn get_local_ping_location() -> Option<String> {
        let u = iface();
        if u.is_null() {
            return None;
        }
        let mut loc: sys::SteamNetworkPingLocation_t = unsafe { std::mem::zeroed() };
        let age = unsafe { sys::SteamAPI_ISteamNetworkingUtils_GetLocalPingLocation(u, &mut loc) };
        if age < 0.0 {
            return None;
        }
        let mut buf = vec![0 as std::os::raw::c_char; sys::k_cchMaxSteamNetworkingPingLocationString as usize];
        unsafe {
            sys::SteamAPI_ISteamNetworkingUtils_ConvertPingLocationToString(
                u,
                &loc,
                buf.as_mut_ptr(),
                buf.len() as i32,
            );
            let s = std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned();
            if s.is_empty() { None } else { Some(s) }
        }
    }

    /// Estimated round trip in ms from this machine to a ping-location
    /// string, or -1 when it can't be parsed or estimated.
    #[napi]
    pub fn estimate_ping_from_local_host(location: String) -> i32 {
        let u = iface();
        if u.is_null() {
            return -1;
        }
        let Ok(c) = std::ffi::CString::new(location) else { return -1 };
        let mut loc: sys::SteamNetworkPingLocation_t = unsafe { std::mem::zeroed() };
        let ok = unsafe { sys::SteamAPI_ISteamNetworkingUtils_ParsePingLocationString(u, c.as_ptr(), &mut loc) };
        if !ok {
            return -1;
        }
        let ms = unsafe { sys::SteamAPI_ISteamNetworkingUtils_EstimatePingTimeFromLocalHost(u, &loc) };
        if ms < 0 { -1 } else { ms }
    }
}
