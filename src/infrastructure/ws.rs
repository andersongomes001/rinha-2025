use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, connect_async};
use tungstenite::{Message};
use crate::domain::entities::HealthStatusAll;
use crate::HealthResponse;
use crate::infrastructure::config::{GLOBAL_HEALTH_STATUS, WS_MASTER_URL};

pub async fn run_master() {
    tokio::spawn(async move {
        let listener = TcpListener::bind("0.0.0.0:9001").await.unwrap();
        println!("[MASTER] WebSocket escutando em ws://0.0.0.0:9001");

        while let Ok((stream, _)) = listener.accept().await {
            println!("[MASTER] Conexão recebida");
            tokio::spawn(async move {
                let ws_stream = accept_async(stream).await.unwrap();
                let (mut write, _read) = ws_stream.split();
                // envio periódico de dados
                loop {
                    let global_status = GLOBAL_HEALTH_STATUS.read().await;
                    let data = HealthStatusAll {
                        default: HealthResponse {
                            failing: global_status.default.failing,
                            min_response_time: global_status.default.min_response_time,
                        },
                        fallback: HealthResponse {
                            failing: global_status.fallback.failing,
                            min_response_time: global_status.fallback.min_response_time,
                        },
                    };
                    let json = serde_json::to_string(&data).unwrap();
                    let _ = write.send(Message::text(json)).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                }
            });
        }
    });
}

pub async fn run_slave() {
    tokio::spawn(async move {
        //let url = url::Url::parse(WS_MASTER_URL.as_str()).unwrap();
        match connect_async(WS_MASTER_URL.as_str()).await {
            Ok((ws_stream, _)) => {
                println!("[CLIENTE] Conectado com sucesso!");
                let (_write, mut read) = ws_stream.split();

                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(msg) => {
                            //println!("{}", msg.clone());
                            if msg.is_text() {
                                let json = msg.to_text().unwrap();
                                match serde_json::from_str::<HealthStatusAll>(json) {
                                    Ok(data) => {
                                        //let debug_data = data.clone();
                                        //println!("default: {:?}, fallback: {:?}", debug_data.default, debug_data.fallback);
                                        GLOBAL_HEALTH_STATUS.write().await.default = data.default;
                                        GLOBAL_HEALTH_STATUS.write().await.fallback = data.fallback;

                                    }
                                    Err(err) => {
                                        eprintln!("[CLIENTE] Erro ao parsear JSON: {err}");
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[CLIENTE] Erro na conexão: {e}");
                            break;
                        }
                    }
                }

                println!("[CLIENTE] Conexão encerrada.");
            }
            Err(e) => {
                eprintln!("[CLIENTE] Falha ao conectar: {e}");
            }
        }
    });
}
