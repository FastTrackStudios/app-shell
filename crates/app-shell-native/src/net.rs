//! What a window's document loads with: what dioxus-native's own launch
//! gives its document (its private `DioxusNativeNetProvider`), so an app
//! on [`crate::launch`] loads exactly as it would on `dioxus::launch`:
//! `asset!()` files (`dioxus://` URLs) from the app's bundle through
//! dioxus' asset resolver, everything else (`http(s):`, `data:`) through
//! blitz-net, waking the event loop when a load lands.

use std::sync::Arc;

use blitz_shell::BlitzShellProxy;
use blitz_traits::net::{NetHandler, NetProvider, Request};

/// The loads of every window of the app.
pub struct AppNetProvider {
    network: Arc<dyn NetProvider>,
}

impl AppNetProvider {
    /// `asset!()` files from the bundle, the rest through blitz-net, which
    /// wakes the event loop behind `proxy`.
    #[must_use]
    pub fn shared(proxy: &BlitzShellProxy) -> Arc<dyn NetProvider> {
        Arc::new(Self {
            network: blitz_net::Provider::shared(Some(Arc::new(proxy.clone()) as _)),
        })
    }
}

impl NetProvider for AppNetProvider {
    fn fetch(&self, doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        if request.url.scheme() == "dioxus" {
            match dioxus_asset_resolver::native::serve_asset(request.url.path()) {
                Ok(res) if res.status().is_success() => {
                    handler.bytes(request.url.to_string(), res.into_body().into());
                }
                Ok(res) => {
                    tracing::warn!(app_shell.asset = %request.url, http.status = res.status().as_u16(), "an asset is not in the bundle");
                }
                Err(error) => {
                    tracing::warn!(app_shell.asset = %request.url, %error, "an asset could not be read");
                }
            }
        } else {
            self.network.fetch(doc_id, request, handler);
        }
    }
}
