use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

pub const ROLE_RECEIVER: u8 = 0;
pub const ROLE_SENDER: u8 = 1;

/// Derives a 16-byte session token from the shared passphrase
pub fn derive_relay_session(passphrase: &str) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"smartxfer_relay_session_v1:");
    hasher.update(passphrase.as_bytes());
    let full = hasher.finalize();
    let mut token = [0u8; 16];
    token.copy_from_slice(&full.as_bytes()[0..16]);
    token
}

/// Connects to a relay as either Receiver (role 0) or Sender (role 1) and waits for pairing
pub fn connect_via_relay(relay_addr: &str, role: u8, passphrase: &str) -> Result<TcpStream> {
    let mut stream = TcpStream::connect(relay_addr)
        .with_context(|| format!("Failed to connect to relay server at {}", relay_addr))?;

    let token = derive_relay_session(passphrase);
    let mut handshake = Vec::with_capacity(17);
    handshake.push(role);
    handshake.extend_from_slice(&token);

    stream.write_all(&handshake)
        .context("Failed to send handshake to relay")?;
    stream.flush()?;

    // Read 1-byte response
    let mut ack = [0u8; 1];
    stream.read_exact(&mut ack)
        .context("Relay disconnected before pairing completed")?;

    match ack[0] {
        1 => Ok(stream),
        code => bail!("Relay rejected connection with status code: {}", code),
    }
}

/// Runs a persistent, lightweight TCP bridging relay
pub fn run_relay(listen_addr: &str) -> Result<()> {
    let listener = TcpListener::bind(listen_addr)
        .with_context(|| format!("Failed to bind relay server to {}", listen_addr))?;

    println!("[smartxfer-relay] Relay listening on {}", listen_addr);
    println!("[smartxfer-relay] Waiting for senders and receivers to connect...");

    let waiting_receivers: Arc<Mutex<HashMap<[u8; 16], TcpStream>>> =
        Arc::new(Mutex::new(HashMap::new()));

    for stream_res in listener.incoming() {
        let mut stream = match stream_res {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[smartxfer-relay] Error accepting connection: {:?}", e);
                continue;
            }
        };

        let waiting = Arc::clone(&waiting_receivers);

        thread::spawn(move || {
            let mut header = [0u8; 17];
            if let Err(e) = stream.read_exact(&mut header) {
                eprintln!("[smartxfer-relay] Handshake read error: {:?}", e);
                return;
            }

            let role = header[0];
            let mut session_id = [0u8; 16];
            session_id.copy_from_slice(&header[1..17]);
            let session_hex = hex_preview(&session_id);

            match role {
                ROLE_RECEIVER => {
                    println!(
                        "[smartxfer-relay] Receiver registered for session {} from {:?}",
                        session_hex,
                        stream.peer_addr().ok()
                    );
                    let mut lock = waiting.lock().unwrap();
                    // Send ACK to receiver
                    if stream.write_all(&[1]).is_ok() {
                        let _ = stream.flush();
                        lock.insert(session_id, stream);
                    }
                }
                ROLE_SENDER => {
                    println!(
                        "[smartxfer-relay] Sender arrived for session {} from {:?}",
                        session_hex,
                        stream.peer_addr().ok()
                    );
                    let receiver_opt = {
                        let mut lock = waiting.lock().unwrap();
                        lock.remove(&session_id)
                    };

                    match receiver_opt {
                        Some(receiver_stream) => {
                            // Send ACK to sender
                            if let Err(e) = stream.write_all(&[1]) {
                                eprintln!("[smartxfer-relay] Failed to ack sender: {:?}", e);
                                return;
                            }
                            let _ = stream.flush();

                            println!(
                                "[smartxfer-relay] Successfully paired session {}. Bridging encrypted streams...",
                                session_hex
                            );
                            bridge_streams(stream, receiver_stream);
                            println!("[smartxfer-relay] Session {} stream closed.", session_hex);
                        }
                        None => {
                            eprintln!(
                                "[smartxfer-relay] Sender connected for session {}, but no receiver is waiting.",
                                session_hex
                            );
                            let _ = stream.write_all(&[2]); // Error code 2: No receiver waiting
                        }
                    }
                }
                _ => {
                    let _ = stream.write_all(&[255]); // Unknown role
                }
            }
        });
    }

    Ok(())
}

fn bridge_streams(mut a: TcpStream, mut b: TcpStream) {
    let mut a_clone = match a.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    };
    let mut b_clone = match b.try_clone() {
        Ok(c) => c,
        Err(_) => return,
    };

    let t1 = thread::spawn(move || {
        let _ = std::io::copy(&mut a, &mut b_clone);
        let _ = b_clone.shutdown(std::net::Shutdown::Write);
    });

    let t2 = thread::spawn(move || {
        let _ = std::io::copy(&mut b, &mut a_clone);
        let _ = a_clone.shutdown(std::net::Shutdown::Write);
    });

    let _ = t1.join();
    let _ = t2.join();
}

fn hex_preview(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}
