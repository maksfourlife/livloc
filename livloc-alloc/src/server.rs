use std::{io, sync::Arc};

use axum::{Router, extract::State, response::IntoResponse, routing::get, serve::Serve};
use livloc_proto::codec::Encode;
use tokio::net::{TcpListener, ToSocketAddrs};

use crate::{LivlocAlloc, tree::Tree};

impl<T> LivlocAlloc<T> {
    pub fn create_router(&self) -> Router {
        #[axum::debug_handler]
        async fn get_tree(state: State<Arc<Tree>>) -> impl IntoResponse {
            let mut bytes = vec![];
            state
                .0
                .to_proto_tree()
                .encode(&mut bytes)
                .expect("infallible");
            bytes
        }

        axum::Router::new()
            .route("/", get(get_tree))
            .with_state(self.tree.clone())
    }

    pub async fn serve(
        &self,
        addrs: impl ToSocketAddrs,
    ) -> io::Result<Serve<TcpListener, Router, Router>> {
        let listener = TcpListener::bind(addrs).await?;
        let serve = axum::serve(listener, self.create_router());
        Ok(serve)
    }
}
