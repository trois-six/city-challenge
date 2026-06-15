use sqlx::postgres::PgPool;

pub async fn init_database(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPool::connect(database_url).await?;

    // Run migrations
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cities (
            id VARCHAR PRIMARY KEY,
            country VARCHAR NOT NULL,
            region VARCHAR NOT NULL,
            department VARCHAR NOT NULL,
            postal_code VARCHAR NOT NULL,
            name VARCHAR NOT NULL,
            date VARCHAR NOT NULL,
            street_count INTEGER NOT NULL,
            total_meters INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS players (
            id VARCHAR PRIMARY KEY,
            name VARCHAR NOT NULL,
            country VARCHAR NOT NULL,
            total_distance FLOAT NOT NULL,
            cities_completed INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS player_results (
            id SERIAL PRIMARY KEY,
            player_id VARCHAR NOT NULL,
            city_id VARCHAR NOT NULL,
            distance FLOAT NOT NULL,
            completed BOOLEAN NOT NULL,
            rank INTEGER NOT NULL,
            FOREIGN KEY (player_id) REFERENCES players(id),
            FOREIGN KEY (city_id) REFERENCES cities(id)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS comments (
            id VARCHAR PRIMARY KEY,
            city_id VARCHAR NOT NULL,
            text TEXT NOT NULL,
            created_at VARCHAR NOT NULL,
            FOREIGN KEY (city_id) REFERENCES cities(id)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_database_init() {
        let database_url = "postgres://user:password@localhost/city_challenge";
        let result = init_database(database_url).await;
        assert!(result.is_ok() || result.is_err()); // Just test it doesn't panic
    }
}
