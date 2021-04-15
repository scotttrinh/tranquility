use {
    crate::error::Error, chrono::NaiveDateTime, ormx::Table, serde_json::Value, sqlx::PgPool,
    uuid::Uuid,
};

#[derive(Clone, Table)]
#[ormx(id = id, table = "actors", insertable)]
pub struct Actor {
    pub id: Uuid,

    pub username: String,
    #[ormx(get_optional(&str))]
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub private_key: Option<String>,

    pub actor: Value,
    pub remote: bool,

    #[ormx(default)]
    pub created_at: NaiveDateTime,
    #[ormx(default)]
    pub updated_at: NaiveDateTime,
}

impl Actor {
    pub async fn by_url(conn_pool: &PgPool, url: &str) -> Result<Self, Error> {
        let actor = sqlx::query_as!(
            Actor,
            r#"
                SELECT * FROM actors
                WHERE actor->>'id' = $1
            "#,
            url
        )
        .fetch_one(conn_pool)
        .await?;

        Ok(actor)
    }

    pub async fn by_username_local(conn_pool: &PgPool, username: &str) -> Result<Self, Error> {
        let actor = sqlx::query_as!(
            Actor,
            r#"
                SELECT * FROM actors
                WHERE username = $1
                AND remote = FALSE
            "#,
            username
        )
        .fetch_one(conn_pool)
        .await?;

        Ok(actor)
    }
}
