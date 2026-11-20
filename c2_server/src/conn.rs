use std::net::SocketAddr;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::mpsc;

pub enum ConnEvent {
    Connected(SocketAddr, tokio::net::tcp::OwnedWriteHalf),
    Disconnected(SocketAddr),
    Output(SocketAddr, Vec<u8>),  // bytes crudos de stdout/stderr del agente
}

pub async fn listen(
    address: &str,
    tx: mpsc::Sender<ConnEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = TcpListener::bind(address).await?;

    loop {
        let (socket, address) = listener.accept().await?;
        let (mut reader, writer) = socket.into_split();

        if tx.send(ConnEvent::Connected(address, writer)).await.is_err() {
            break;
        }

        let tx_clone = tx.clone();

        tokio::spawn(async move {
            // 4 KB: suficiente para la mayoría de outputs de shell sin fragmentar demasiado
            let mut buf = [0u8; 4096];

            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => {
                        // FIN ordenado: el agente cerró la conexión
                        break;
                    }
                    Ok(n) => {
                        // Copiamos solo los bytes leídos y los mandamos al main loop
                        let data = buf[..n].to_vec();
                        if tx_clone.send(ConnEvent::Output(address, data)).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        // Error de red: desconexión abrupta
                        break;
                    }
                }
            }

            let _ = tx_clone.send(ConnEvent::Disconnected(address)).await;
        });
    }

    Ok(())
}