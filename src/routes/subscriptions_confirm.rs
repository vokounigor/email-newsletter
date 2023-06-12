use actix_web::{web, HttpResponse, ResponseError};
use anyhow::Context;
use reqwest::StatusCode;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[derive(thiserror::Error)]
pub enum SubscribeConfirmError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeConfirmError {
    fn status_code(&self) -> reqwest::StatusCode {
        match self {
            SubscribeConfirmError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
pub async fn confirm(
    parameters: web::Query<Parameters>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, SubscribeConfirmError> {
    let id = get_subscriber_id_from_token(&parameters.subscription_token, &pool)
        .await
        .context("Error finding subscriber from token")?;
    match id {
        None => Ok(HttpResponse::Unauthorized().finish()),
        Some(subscriber_id) => {
            if is_user_confirmed(subscriber_id, &pool).await {
                return Ok(HttpResponse::Ok().finish());
            }
            let mut transaction = pool
                .begin()
                .await
                .context("Fialed to acquire a Postgres connection from the pool")?;
            confirm_subscriber(subscriber_id, &mut transaction)
                .await
                .context("Failed to set subscriber status to confirmed")?;
            delete_old_token(subscriber_id, &mut transaction)
                .await
                .context("Failed to delete old subscriber token")?;
            transaction
                .commit()
                .await
                .context("Failed to commit SQL transaction to confirm user")?;
            Ok(HttpResponse::Ok().finish())
        }
    }
}

#[tracing::instrument(name = "Get subscriber_id from token", skip(subscription_token, pool))]
pub async fn get_subscriber_id_from_token(
    subscription_token: &str,
    pool: &PgPool,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1",
        subscription_token
    )
    .fetch_optional(pool)
    .await?;
    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(
    name = "Mark subscriber as confirmed",
    skip(subscriber_id, transaction)
)]
pub async fn confirm_subscriber(
    subscriber_id: Uuid,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id
    )
    .execute(transaction)
    .await?;
    Ok(())
}

#[tracing::instrument(
    name = "Deleting old token from subscription_tokens",
    skip(subscriber_id, transaction)
)]
pub async fn delete_old_token(
    subscriber_id: Uuid,
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM subscription_tokens WHERE subscriber_id = $1",
        subscriber_id
    )
    .execute(transaction)
    .await?;
    Ok(())
}

#[tracing::instrument(
    name = "Checking if user is already confirmed",
    skip(subscriber_id, pool)
)]
pub async fn is_user_confirmed(subscriber_id: Uuid, pool: &PgPool) -> bool {
    sqlx::query!(
        "SELECT status FROM subscriptions WHERE id = $1 AND status = 'confirmed'",
        subscriber_id
    )
    .fetch_one(pool)
    .await
    .is_ok()
}

fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
