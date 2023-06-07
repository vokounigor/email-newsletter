use actix_web::{web, HttpResponse};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(parameters, pool))]
pub async fn confirm(parameters: web::Query<Parameters>, pool: web::Data<PgPool>) -> HttpResponse {
    let id = match get_subscriber_id_from_token(&parameters.subscription_token, &pool).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };
    match id {
        None => HttpResponse::Unauthorized().finish(),
        Some(subscriber_id) => {
            if is_user_confirmed(subscriber_id, &pool).await {
                return HttpResponse::Ok().finish();
            }
            let mut transaction = match pool.begin().await {
                Ok(transaction) => transaction,
                Err(_) => return HttpResponse::InternalServerError().finish(),
            };
            if confirm_subscriber(subscriber_id, &mut transaction)
                .await
                .is_err()
            {
                return HttpResponse::InternalServerError().finish();
            }

            if delete_old_token(subscriber_id, &mut transaction)
                .await
                .is_err()
            {
                return HttpResponse::InternalServerError().finish();
            }

            if transaction.commit().await.is_err() {
                return HttpResponse::InternalServerError().finish();
            }
            HttpResponse::Ok().finish()
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query {:?}", e);
        e
    })?;
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query {:?}", e);
        e
    })?;
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query {:?}", e);
        e
    })?;
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
