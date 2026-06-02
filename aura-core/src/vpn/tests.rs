#[cfg(test)]
mod tests {
    use crate::vpn::{OpenVpnProvider, VpnProvider, VpnStatus, WireGuardProvider};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_wireguard_provider_not_found_error() {
        // Create a provider with a dummy interface
        let provider = WireGuardProvider::new("wg_mock_dummy".to_string());

        // Attempting status, connect, or disconnect should fail elegantly with NotFound,
        // or since "wg" command doesn't exist under standard test path or if it does,
        // it should either work or return our custom NotFound helper error.
        let status_res = provider.status().await;
        // The mock provider is designed to return Disconnected on CLI error, which is safe.
        assert_eq!(status_res.unwrap(), VpnStatus::Disconnected);

        // Connect should fail if wg-quick is not in PATH
        let conn_res = provider.connect().await;
        if let Err(e) = conn_res {
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("is not installed")
                    || err_msg.contains("wg-quick up failed")
                    || err_msg.contains("timed out")
            );
        }
    }

    #[tokio::test]
    async fn test_openvpn_provider_handshake_and_state() {
        // Start a mock OpenVPN management server
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        // Spawn mock server handler
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                // 1. Send OpenVPN management greeting banner
                let greeting = ">INFO:OpenVPN Management Interface Version 1\n";
                socket.write_all(greeting.as_bytes()).await.unwrap();

                // 2. Read command sent by provider
                let mut reader = BufReader::new(socket);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();

                if line.trim() == "state" {
                    // Send response terminated by END
                    let state_response = "1622300000,CONNECTED,SUCCESS,10.8.0.6,192.0.2.1\nEND\n";
                    let mut socket = reader.into_inner();
                    socket.write_all(state_response.as_bytes()).await.unwrap();
                }
            }
        });

        // Instantiate OpenVpnProvider connecting to mock server
        let provider = OpenVpnProvider::new(local_addr.to_string());

        // Verify status check succeeds and parses correctly
        let status = provider.status().await.unwrap();
        assert_eq!(status, VpnStatus::Connected);
    }

    #[tokio::test]
    async fn test_openvpn_provider_password_authentication() {
        // Start a mock OpenVPN management server requiring password
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let local_addr = listener.local_addr().unwrap();

        // Spawn mock server handler
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                // 1. Send welcome banner and ENTER PASSWORD challenge
                let greeting = ">INFO:OpenVPN Management Interface\nENTER PASSWORD:\n";
                socket.write_all(greeting.as_bytes()).await.unwrap();

                // 2. Read password from client
                let mut reader = BufReader::new(socket);
                let mut password_line = String::new();
                reader.read_line(&mut password_line).await.unwrap();

                if password_line.trim() == "my_secret_pass" {
                    // Send success response
                    let auth_success = "SUCCESS: password is correct\n>INFO: authenticated\n";
                    let mut socket = reader.into_inner();
                    socket.write_all(auth_success.as_bytes()).await.unwrap();

                    // Read state command
                    let mut reader = BufReader::new(socket);
                    let mut cmd_line = String::new();
                    reader.read_line(&mut cmd_line).await.unwrap();

                    if cmd_line.trim() == "state" {
                        let state_response = "1622300000,CONNECTING,WAIT,,,\nEND\n";
                        let mut socket = reader.into_inner();
                        socket.write_all(state_response.as_bytes()).await.unwrap();
                    }
                }
            }
        });

        // Instantiate OpenVpnProvider with password
        let provider = OpenVpnProvider::new(local_addr.to_string())
            .with_password("my_secret_pass".to_string());

        // Verify status parses CONNECTING correctly
        let status = provider.status().await.unwrap();
        assert_eq!(status, VpnStatus::Connecting);
    }
}
