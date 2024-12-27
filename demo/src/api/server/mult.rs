use terrazzo::axum::Json;

pub async fn mult(Json((a, b)): Json<(i32, i32)>) -> Json<i32> {
    Json(a * b)
}
