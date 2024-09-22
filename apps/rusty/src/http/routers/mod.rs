use std::{path::PathBuf, sync::Arc};

use authentication::create_authentication_router;
use lobby::create_lobby_router;

use crate::Ctx;

mod authentication;
mod lobby;

pub fn create_router() -> Arc<rspc::Router<Ctx>> {
    let router = rspc::Router::<Ctx>::new()
        .query("version", |t| t(|ctx, input: ()| env!("CARGO_PKG_VERSION")))
        .merge("authentication.", create_authentication_router())
        .merge("lobby.", create_lobby_router())
        .build()
        .arced();

    // prob can just be a command ? leaving it here for now
    router
        .export_ts(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bindings.ts"))
        .expect("Unable to export ts bindings.");

    router
}
