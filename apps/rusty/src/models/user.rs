use bcrypt::verify;
use sqlx::{query, query_as, Pool, Postgres};
use uuid::Uuid;

use super::error::{ModelError, ModelResult};

#[derive(Debug)]
pub struct User {
    id: String,
    password: String,
}

impl User {
    pub async fn find_by_refresh_token(
        pool: &Pool<Postgres>,
        (user_id, token): (&String, &String),
    ) -> ModelResult<User> {
        query_as!(User, "select id, password from users join refresh_tokens on refresh_tokens.user_id = users.id where users.id = $1 and refresh_tokens.token = $2", user_id, token)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::SqlError(e.to_string()))
    }

    pub async fn find(pool: &Pool<Postgres>, id: &String) -> ModelResult<User> {
        query_as!(User, "select id, password from users where id = $1", id)
            .fetch_one(pool)
            .await
            .map_err(|e| ModelError::SqlError(e.to_string()))
    }

    pub async fn create_refresh_token(self: &User, pool: &Pool<Postgres>) -> ModelResult<String> {
        let token = Uuid::new_v4().to_string();
        query!(
            "insert into refresh_tokens (token, user_id) values ($1, $2)",
            token,
            self.id
        )
        .execute(pool)
        .await
        .map_err(|e| ModelError::SqlError(e.to_string()))?;

        Ok(token)
    }

    pub fn verify_password(self: &User, password: &String) -> bool {
        verify(password, &self.password).unwrap_or_default()
    }

    pub fn get_id(&self) -> &String {
        return &self.id;
    }
}
