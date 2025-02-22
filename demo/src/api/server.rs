use terrazzo::axum::Router;
use terrazzo::axum::routing::post;

mod mult;

pub fn route() -> Router {
    Router::new().route("/mult", post(mult::mult))
}
