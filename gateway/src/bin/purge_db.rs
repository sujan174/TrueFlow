use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost/trueflow".to_string());

    println!("Connecting to {}", database_url);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    println!("Resetting database schema...");
    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE SCHEMA public").execute(&pool).await?;

    println!("Running migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    println!("Database reset successfully.");
    Ok(())
}
