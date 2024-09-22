use std::sync::Arc;

use rspc::Router;

use crate::{
    http::controllers::authentication::{AuthenticationController, LoginArgs},
    Ctx,
};

pub fn create_authentication_router() -> rspc::RouterBuilder<Ctx> {
    <Router<Ctx>>::new()
        .mutation("refresh_token", |t| {
            t(|ctx, token: String| async move {
                Ok(AuthenticationController::refresh_token(ctx, token).await?)
            })
        })
        .mutation("login", |t| {
            t(|ctx, args: LoginArgs| async move {
                Ok(AuthenticationController::login(ctx, args).await?)
            })
        })
}
