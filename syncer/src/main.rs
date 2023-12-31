#[tokio::main(flavor = "current_thread")]
async fn main() {
    anki::log::set_global_logger(None).unwrap();

    let coll_path = std::env::var("COLLECTION_PATH").expect("COLLECTION_PATH not set");
    let host = std::env::var("ANKI_HOST").expect("ANKI_HOST not set");
    let username = std::env::var("ANKI_USERNAME").expect("ANKI_USERNAME not set");
    let password = std::env::var("ANKI_PASSWORD").expect("ANKI_PASSWORD not set");

    let authenticate = |endpoint: String| async {
        anki::sync::login::SyncAuth {
            endpoint: endpoint.parse().ok(),
            ..anki::sync::login::sync_login(
                &username,
                &password,
                Some(endpoint),
                Default::default(),
            )
            .await
            .expect("login failed")
        }
    };

    let reauthenticate = |prior: anki::sync::login::SyncAuth, new_endpoint: Option<String>| async {
        if let Some(new_endpoint) = new_endpoint {
            authenticate(new_endpoint).await
        } else {
            prior
        }
    };

    let (full_sync_required, sync_auth) = {
        let mut collection = anki::collection::CollectionBuilder::new(&coll_path)
            .build()
            .expect("failed to build collection");

        let sync_auth = authenticate(host).await;

        match collection
            .normal_sync(sync_auth.clone(), Default::default())
            .await
        {
            Ok(anki::sync::collection::normal::SyncOutput {
                required:
                    anki::sync::collection::normal::SyncActionRequired::FullSyncRequired { .. },
                new_endpoint,
                ..
            }) => {
                download(collection, reauthenticate(sync_auth.clone(), new_endpoint).await).await;
                (true, sync_auth)
            }
            Ok(anki::sync::collection::normal::SyncOutput {
                required: anki::sync::collection::normal::SyncActionRequired::NoChanges,
                new_endpoint,
                ..
            }) => {
                let local = collection
                    .sync_meta()
                    .expect("failed to get local sync meta");
                let downloaded = if local.usn == 0i32.into() || local.modified == 0i64.into() {
                    tracing::warn!(
                        "anki server reported no changes required, but the collection appears empty. downloading from {}", new_endpoint.as_deref().unwrap_or("original endpoint")
                    );
                    download(collection, reauthenticate(sync_auth.clone(), new_endpoint).await).await;
                    true
                } else {
                    false
                };
                (downloaded, sync_auth)
            }
            Ok(_) => (false, sync_auth),
            Err(
                e @ anki::error::AnkiError::SyncError {
                    source:
                        anki::error::SyncError {
                            kind:
                                anki::error::SyncErrorKind::Conflict
                                | anki::error::SyncErrorKind::ResyncRequired
                                | anki::error::SyncErrorKind::DatabaseCheckRequired,
                            ..
                        },
                },
            ) => {
                tracing::error!("failed to normal sync due to error: {e}");
                let local = collection
                    .sync_meta()
                    .expect("failed to get local sync meta");
                let mut sync_client = anki::sync::http_client::HttpSyncClient::new(
                    sync_auth.clone(),
                    Default::default(),
                );
                let status = anki::sync::collection::status::online_sync_status_check(
                    local,
                    &mut sync_client,
                )
                .await
                .expect("failed to online status check");
                let sync_auth = reauthenticate(sync_auth, status.new_endpoint).await;
                download(collection, sync_auth.clone()).await;
                (true, sync_auth)
            }
            Err(e) => panic!("failed to normal sync: {:?}", e),
        }
    };

    // Re-open the collection again to upgrade schema.
    for _ in 0..3 {
        let mut collection = anki::collection::CollectionBuilder::new(&coll_path)
            .build()
            .expect("failed to re-build collection");
        match anki::services::CollectionService::check_database(&mut collection) {
            Ok(_) => break,
            Err(e) => {
                tracing::error!("collection database check failed {e}");
                download(collection, sync_auth.clone()).await;
            }
        }
    }

    std::process::exit(if full_sync_required { 255 } else { 0 });
}

async fn download(collection: anki::collection::Collection, auth: anki::sync::login::SyncAuth) {
    collection
        .full_download(auth, Default::default())
        .await
        .expect("full download failed");
}
