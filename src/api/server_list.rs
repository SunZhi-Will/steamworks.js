use napi_derive::napi;

/// Client side of the dedicated server: find game servers on the Steam master
/// server list by a filter (e.g. `gametagsand` = the room's invite-code tag).
///
/// Polling instead of callbacks/promises: `request` starts a query and returns
/// an id; `take` returns what has arrived so far and whether the query is done.
/// Results arrive while the client's callbacks run (steamworks.js already runs
/// them on a timer in the same thread).
#[napi]
#[allow(non_snake_case)]
pub mod serverList {
    use napi::bindgen_prelude::BigInt;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex};
    use steamworks::{AppId, ServerListCallbacks, ServerListRequest};

    #[napi(object)]
    #[derive(Clone)]
    pub struct FoundServer {
        pub steam_id64: BigInt,
        pub name: String,
        pub tags: String,
        pub players: i32,
        pub max_players: i32,
        pub responded: bool,
    }

    #[napi(object)]
    pub struct ServerListResult {
        pub done: bool,
        pub servers: Vec<FoundServer>,
    }

    struct Pending {
        found: Rc<RefCell<Vec<FoundServer>>>,
        done: Rc<RefCell<bool>>,
        request: Option<Arc<Mutex<ServerListRequest>>>,
    }

    thread_local! {
        static PENDING: RefCell<HashMap<u32, Pending>> = RefCell::new(HashMap::new());
        static NEXT_ID: RefCell<u32> = const { RefCell::new(1) };
    }

    fn details(req: &Arc<Mutex<ServerListRequest>>, index: i32, responded: bool) -> Option<FoundServer> {
        let item = req.lock().ok()?.get_server_details(index).ok()?;
        Some(FoundServer {
            steam_id64: BigInt::from(item.steamid.raw()),
            name: item.server_name,
            tags: item.tags,
            players: item.players,
            max_players: item.max_players,
            responded,
        })
    }

    /// Starts an internet server list query for `appId` with `filters`
    /// (pairs of [key, value], see ISteamMatchmakingServers). Returns 0 when it
    /// could not start (no Steam client, bad filter).
    #[napi]
    pub fn request(app_id: u32, filters: Vec<Vec<String>>) -> u32 {
        if !crate::client::has_client() {
            return 0;
        }
        let client = crate::client::get_client();
        let found: Rc<RefCell<Vec<FoundServer>>> = Rc::new(RefCell::new(Vec::new()));
        let done: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        let (f1, f2, d) = (found.clone(), found.clone(), done.clone());
        let callbacks = ServerListCallbacks::new(
            Box::new(move |req, server| {
                if let Some(s) = details(&req, server, true) { f1.borrow_mut().push(s); }
            }),
            // Behind a home router the server may not answer the browser's
            // ping, but the master server still knows its Steam ID and tags.
            Box::new(move |req, server| {
                if let Some(s) = details(&req, server, false) { f2.borrow_mut().push(s); }
            }),
            Box::new(move |_req, _response| { *d.borrow_mut() = true; }),
        );
        let pairs: Vec<(String, String)> = filters
            .into_iter()
            .filter(|p| p.len() == 2)
            .map(|p| (p[0].clone(), p[1].clone()))
            .collect();
        let map: HashMap<&str, &str> = pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let request = match client.matchmaking_servers().internet_server_list(AppId(app_id), &map, callbacks) {
            Ok(r) => r,
            Err(_) => return 0,
        };
        let id = NEXT_ID.with(|n| { let v = *n.borrow(); *n.borrow_mut() = v + 1; v });
        PENDING.with(|p| p.borrow_mut().insert(id, Pending { found, done, request: Some(request) }));
        id
    }

    /// What arrived so far. Once `done` is true the request is released.
    #[napi]
    pub fn take(id: u32) -> ServerListResult {
        PENDING.with(|p| {
            let mut map = p.borrow_mut();
            let Some(entry) = map.get_mut(&id) else {
                return ServerListResult { done: true, servers: Vec::new() };
            };
            let servers: Vec<FoundServer> = entry.found.borrow_mut().drain(..).collect();
            let done = *entry.done.borrow();
            if done {
                if let Some(req) = entry.request.take() {
                    if let Ok(mut r) = req.lock() { let _ = r.release(); }
                }
                map.remove(&id);
            }
            ServerListResult { done, servers }
        })
    }

    /// Gives up on a query that is still running.
    #[napi]
    pub fn cancel(id: u32) {
        PENDING.with(|p| {
            if let Some(mut entry) = p.borrow_mut().remove(&id) {
                if let Some(req) = entry.request.take() {
                    if let Ok(mut r) = req.lock() { let _ = r.release(); }
                }
            }
        });
    }
}
