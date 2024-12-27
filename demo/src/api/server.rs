use terrazzo::axum::routing::post;
use terrazzo::axum::Router;

mod mult;

pub fn route() -> Router {
    Router::new().route("/mult", post(mult::mult))
}
