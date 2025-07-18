use chrono::{SecondsFormat, Utc};
use redis::aio::{ConnectionManager, MultiplexedConnection};
use redis::{AsyncCommands};
use reqwest::{Client, StatusCode};
use rinha2025::{get_redis_connection, process, PostPayments, QUEUE_FAILED_KEY, QUEUE_KEY};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Semaphore};

const MAX_CURRENT_JOB: usize = 5000;

#[tokio::main]
async fn main() {
    let connection: Arc<ConnectionManager> = match get_redis_connection().await {
        Ok(conn) => Arc::new(conn),
        Err(e) => {
            eprintln!("Failed to connect to Redis: {:?}", e);
            return;
        }
    };
    let client = Arc::new(Client::builder()
        .timeout(Duration::from_millis(300))
        .build()
        .unwrap());
    let semaphore = Arc::new(Semaphore::new(MAX_CURRENT_JOB));

    //reprocessar
    /*
    let connection_clone = Arc::clone(&connection);
    tokio::spawn(async move {
        loop {
            let mut conn = connection_clone.lock().await;
            match AsyncCommands::lrange::<_, Vec<String>>(&mut *conn, QUEUE_FAILED_KEY, 0, -1).await {
                Ok(items) => {
                    for item in items {
                        let retry_key = format!("retries:{}", &item);
                        let retries: i32 = AsyncCommands::incr(&mut *conn, &retry_key, 1).await.unwrap_or(1);

                        if retries <= 3 {
                            if process(item, conn, client).await.is_ok() {
                                let _: () = AsyncCommands::lrem(&mut *conn, QUEUE_FAILED_KEY, 1, &item).await?;
                                let _: () = AsyncCommands::del(&mut *conn, &retry_key).await?;
                            } else {
                                eprintln!("Erro ao reprocessar item da fila");
                            }
                        } else {
                            let _: () = AsyncCommands::lrem(&mut *conn, QUEUE_FAILED_KEY, 1, &item).await?;
                            let _: () = AsyncCommands::del(&mut *conn, &retry_key).await?;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("erro ao reprocessar fila: {:?}", e);
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });*/


    loop {
        let mut conn =(*connection).clone();
        let semaphore = Arc::clone(&semaphore);
        match conn.brpop::<_,Vec<String>>(QUEUE_KEY, 0.0).await {
            Ok(result) => {
                if let Some(payload) = result.get(1){
                    let client = Arc::clone(&client);
                    let permit = semaphore.acquire_owned().await.unwrap();
                    let payload = payload.clone();

                    let conn_clone = (*connection).clone();
                    tokio::spawn(async move {
                        let _ = process(payload, conn_clone, client).await;
                        drop(permit);
                    });
                }
            }
            Err(e) => {
                eprintln!("Erro ao buscar item da fila: {:?}", e);
            },
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}


