#[tokio::main(flavor = "current_thread")]
async fn main() {
    anki::log::set_global_logger(None).unwrap();

    let coll_path = std::env::var("COLLECTION_PATH").expect("COLLECTION_PATH not set");
    let host = std::env::var("ANKI_HOST").expect("ANKI_HOST not set");
    let username = std::env::var("ANKI_USERNAME").expect("ANKI_USERNAME not set");
    let password = std::env::var("ANKI_PASSWORD").expect("ANKI_PASSWORD not set");

    std::env::set_var("SYNC_ENDPOINT", host);

    let mut collection = anki::collection::CollectionBuilder::new(coll_path.clone())
        .build()
        .expect("failed to build collection");

    let sync_auth = anki::sync::sync_login(&username, &password)
        .await
        .expect("login failed");

    if let anki::sync::SyncOutput {
        required: anki::sync::SyncActionRequired::FullSyncRequired { .. },
        ..
    } = collection
        .normal_sync(sync_auth.clone(), Box::new(|_, _| {}))
        .await
        .expect("normal sync failed")
    {
        collection
            .full_download(sync_auth, Box::new(|_, _| {}))
            .await
            .expect("full download failed");
    } else {
        drop(collection);
    }

    // Open the collection again to upgrade schema.
    anki::collection::CollectionBuilder::new(coll_path)
        .build()
        .expect("failed to re-build collection");
}
