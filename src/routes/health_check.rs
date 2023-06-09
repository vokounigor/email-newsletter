use actix_web::{HttpResponse, Responder};

#[allow(clippy::all)]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}
