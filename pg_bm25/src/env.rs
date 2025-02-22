use once_cell::sync::Lazy;
use pgrx::{register_xact_callback, PgXactCallbackEvent};
use std::{
    collections::HashSet,
    ffi::CStr,
    panic::{RefUnwindSafe, UnwindSafe},
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};
use thiserror::Error;
use tracing::error;

use crate::writer::{WriterClient, WriterRequest};

const TRANSACTION_CALLBACK_CACHE_ID: &str = "parade_search_index";

static TRANSACTION_CALL_ONCE_ON_COMMIT_CACHE: Lazy<Arc<Mutex<HashSet<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::new())));

static TRANSACTION_CALL_ONCE_ON_ABORT_CACHE: Lazy<Arc<Mutex<HashSet<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashSet::new())));

/// We use this global variable to cache any values that can be re-used
/// after initialization.
static SEARCH_ENV: Lazy<SearchEnv> = Lazy::new(|| SearchEnv {
    postgres_data_dir: Mutex::new(None),
    postgres_database_oid: Mutex::new(None),
});

struct SearchEnv {
    postgres_data_dir: Mutex<Option<PathBuf>>,
    postgres_database_oid: Mutex<Option<u32>>,
}

pub fn postgres_data_dir_path() -> PathBuf {
    SEARCH_ENV
        .postgres_data_dir
        .lock()
        .expect("Failed to lock mutex")
        .get_or_insert_with(|| unsafe {
            let data_dir = CStr::from_ptr(pgrx::pg_sys::DataDir)
                .to_string_lossy()
                .into_owned();
            PathBuf::from(data_dir)
        })
        .clone()
}

pub fn postgres_database_oid() -> u32 {
    *SEARCH_ENV
        .postgres_database_oid
        .lock()
        .expect("Failed to lock mutex")
        .get_or_insert_with(|| unsafe { pgrx::pg_sys::MyDatabaseId.as_u32() })
}

pub struct Transaction {}

impl Transaction {
    pub fn needs_commit(id: &str) -> Result<bool, TransactionError> {
        let cache = TRANSACTION_CALL_ONCE_ON_COMMIT_CACHE.lock()?;
        Ok(cache.contains(id))
    }

    pub fn call_once_on_precommit<F>(id: &str, callback: F) -> Result<(), TransactionError>
    where
        F: FnOnce() + Send + UnwindSafe + RefUnwindSafe + 'static,
    {
        // Clone the cache here for use inside the closure.
        let cache_clone = TRANSACTION_CALL_ONCE_ON_COMMIT_CACHE.clone();

        let mut cache = TRANSACTION_CALL_ONCE_ON_COMMIT_CACHE.lock()?;
        if !cache.contains(id) {
            // Now using `cache_clone` inside the closure.
            register_xact_callback(PgXactCallbackEvent::PreCommit, move || {
                // Clear the cache so callbacks can be registered on next transaction.
                match cache_clone.lock() {
                    Ok(mut cache) => cache.clear(),
                    Err(err) => error!(
                        "could not acquire lock in register transaction commit callback: {err:?}"
                    ),
                }

                // Actually call the callback.
                callback();
            });

            cache.insert(id.into());
        }

        Ok(())
    }

    pub fn call_once_on_abort<F>(id: &str, callback: F) -> Result<(), TransactionError>
    where
        F: FnOnce() + Send + UnwindSafe + RefUnwindSafe + 'static,
    {
        // Clone the cache here for use inside the closure.
        let cache_clone = TRANSACTION_CALL_ONCE_ON_ABORT_CACHE.clone();

        let mut cache = TRANSACTION_CALL_ONCE_ON_ABORT_CACHE.lock()?;
        if !cache.contains(id) {
            // Now using `cache_clone` inside the closure.
            register_xact_callback(PgXactCallbackEvent::Abort, move || {
                // Clear the cache so callbacks can be registered on next transaction.
                match cache_clone.lock() {
                    Ok(mut cache) => cache.clear(),
                    Err(err) => error!(
                        "could not acquire lock in register transaction abort callback: {err:?}"
                    ),
                }

                // Actually call the callback.
                callback();
            });

            cache.insert(id.into());
        }

        Ok(())
    }
}

pub fn register_commit_callback<W: WriterClient<WriterRequest> + Send + Sync + 'static>(
    writer: &Arc<Mutex<W>>,
) -> Result<(), TransactionError> {
    let writer_client = writer.clone();
    Transaction::call_once_on_precommit(
        TRANSACTION_CALLBACK_CACHE_ID,
        move || match writer_client.lock() {
            Err(err) => {
                pgrx::log!("could not lock  client in commit callback: {err}");
            }
            Ok(mut client) => client.request(WriterRequest::Commit).unwrap_or_else(|err| {
                pgrx::log!("error sending commit request in callback: {err}")
            }),
        },
    )?;

    let writer_client = writer.clone();
    Transaction::call_once_on_abort(TRANSACTION_CALLBACK_CACHE_ID, move || {
        match writer_client.lock() {
            Err(err) => {
                pgrx::log!("could not lock  client in abort callback: {err}");
            }
            Ok(mut client) => client
                .request(WriterRequest::Abort)
                .unwrap_or_else(|err| pgrx::log!("error sending abort request in callback: {err}")),
        }
    })?;

    Ok(())
}

pub fn needs_commit() -> bool {
    Transaction::needs_commit(TRANSACTION_CALLBACK_CACHE_ID)
        .expect("error performing commit check in transaction cache")
}

#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("could not acquire lock in transaction callback")]
    AcquireLock,
}

impl<T> From<PoisonError<T>> for TransactionError {
    fn from(_: PoisonError<T>) -> Self {
        TransactionError::AcquireLock
    }
}
