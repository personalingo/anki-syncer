#[tokio::main(flavor = "current_thread")]
async fn main() {
    anki::log::set_global_logger(None).unwrap();

    let coll_path = std::env::var("COLLECTION_PATH").expect("COLLECTION_PATH not set");
    let host = std::env::var("ANKI_HOST").expect("ANKI_HOST not set");
    let username = std::env::var("ANKI_USERNAME").expect("ANKI_USERNAME not set");
    let password = std::env::var("ANKI_PASSWORD").expect("ANKI_PASSWORD not set");

    let mut collection = anki::collection::CollectionBuilder::new(coll_path.clone())
        .set_no_prepopulate(true) // Don't create default decks and notetypes or the sanity check will fail.
        .build()
        .expect("failed to build collection");

    let sync_auth = anki::sync::login::sync_login(username, password, Some(host), Default::default())
        .await
        .expect("login failed");

    collection
        .normal_sync(sync_auth, Default::default())
        .await
        .expect("failed to sync");

    // Re-open the collection again to upgrade schema.
    drop(collection);
    anki::collection::CollectionBuilder::new(coll_path)
        .build()
        .expect("failed to re-build collection");
}
