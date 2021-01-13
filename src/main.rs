use model_testing::{api, in_memory_db};

#[async_std::main]
async fn main() -> anyhow::Result<()> {
    let db = in_memory_db::init_db();
    let mut app = tide::with_state(db);
    app.at("/login").post(api::login);
    app.at("/logout").post(api::logout);
    app.at("/secret/:user").get(api::secret);
    Ok(())
}
