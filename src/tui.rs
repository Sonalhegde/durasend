use anyhow::Result;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::crypto;
use crate::installer;
use crate::runner::{run_receive_action, run_send_action};
use crate::theme::{self, Theme};

pub fn run_interactive_ui() -> Result<()> {
    theme::enable_ansi_support();

    loop {
        let theme = theme::next_theme();
        print_menu_banner(theme);

        println!("  {} {} {}", theme.accent.bold("[1]"), "\x1b[1;97mReceive a file\x1b[0m", theme.muted.fg("(Direct LAN or UPnP)"));
        println!("  {} {} {}", theme.accent.bold("[2]"), "\x1b[1;97mSend a file\x1b[0m", theme.muted.fg("(Direct connection)"));
        println!("  {} {} {}", theme.accent.bold("[3]"), "\x1b[1;97mReceive via Remote Relay\x1b[0m", theme.muted.fg("(Across firewalls / NAT)"));
        println!("  {} {} {}", theme.accent.bold("[4]"), "\x1b[1;97mSend via Remote Relay\x1b[0m", theme.muted.fg("(Across firewalls / NAT)"));
        println!("  {} {} {}", theme.accent.bold("[5]"), "\x1b[1;97mStart a Relay Server\x1b[0m", theme.muted.fg("(Zero-knowledge bridge)"));
        println!("  {} {} {}", theme.accent.bold("[6]"), "\x1b[1;97mInstall SmartXfer to PATH\x1b[0m", theme.muted.fg("(Make globally executable)"));
        println!("  {} {} {}", theme.accent.bold("[7]"), "\x1b[1;97mRun Self-Test & Diagnostic\x1b[0m", theme.muted.fg("(Verify AEAD & BLAKE3)"));
        println!("  {} {} {}", theme.accent.bold("[0]"), "\x1b[1;97mCycle RGB Theme Palette\x1b[0m", theme.muted.fg("(Shift to next colors)"));
        println!("  {} {}", theme.border.bold("[8]"), "\x1b[90mExit\x1b[0m");
        println!();
        print!("{} Select an option {}: ", theme.c1.bold(">>"), theme.accent.bold("[0-8]"));
        io::stdout().flush()?;

        let choice = read_input()?.trim().to_string();
        println!();

        match choice.as_str() {
            "1" => handle_direct_receive(theme)?,
            "2" => handle_direct_send(theme)?,
            "3" => handle_relay_receive(theme)?,
            "4" => handle_relay_send(theme)?,
            "5" => handle_start_relay(theme)?,
            "6" => {
                installer::install_binary()?;
            }
            "7" => run_self_test(theme)?,
            "0" | "c" | "theme" | "color" => {
                // Loops and automatically picks the next theme!
                continue;
            }
            "8" | "q" | "exit" => {
                println!("{} Exiting SmartXfer.", theme::tag_info());
                break;
            }
            _ => {
                println!("{} Invalid choice. Please select 0 through 8.", theme::tag_warn());
            }
        }
    }

    Ok(())
}

fn print_menu_banner(theme: &Theme) {
    let banner_raw = [
        r"  ____  __  __    _    ____ _____ __  ______ _____ ____  ",
        r" / ___||  \/  |  / \  |  _ \_   _|\ \/ /  ___| ____|  _ \ ",
        r" \___ \| |\/| | / _ \ | |_) || |   \  /| |_  |  _| | |_) |",
        r"  ___) | |  | |/ ___ \|  _ < | |   /  \|  _|| |___|  _ < ",
        r" |____/|_|  |_/_/   \_\_| \_\|_|  /_/\_\_|   |_____|_| \_\",
    ];

    let art_lines = theme::render_ascii_art_gradient(&banner_raw, theme.c1, theme.c2, theme.c3);

    let border_h = "+============================================================+";
    let divider_h = "|------------------------------------------------------------|";

    println!();
    println!("{}", theme.border.bold(border_h));
    for art_line in art_lines {
        println!("{} {}  {}", theme.border.fg("|"), art_line, theme.border.fg("|"));
    }
    println!("{}", theme.border.fg(divider_h));

    let subtitle = "    Resilient - Encrypted - Autonomous File Transfer Suite    ";
    let colored_sub = theme::gradient_3stop(subtitle, theme.c1, theme.c2, theme.c3);
    println!("{} {} {}", theme.border.fg("|"), colored_sub, theme.border.fg("|"));

    let palette_info = format!(" [ RGB Palette: {} ] ", theme.name);
    let padded_info = format!("{:^60}", palette_info);
    let colored_info = theme.accent.bold(&padded_info);
    println!("{} {} {}", theme.border.fg("|"), colored_info, theme.border.fg("|"));

    println!("{}", theme.border.bold(border_h));
    println!();
}

fn handle_direct_receive(theme: &Theme) -> Result<()> {
    println!("{}", theme.border.bold("+--- [1] Receive a File (Direct LAN / UPnP) ---+"));
    print!("{} Bind address [default: 0.0.0.0:9099]: ", theme::tag_prompt());
    io::stdout().flush()?;
    let mut listen = read_input()?.trim().to_string();
    if listen.is_empty() {
        listen = "0.0.0.0:9099".to_string();
    }

    print!("{} Save files into directory [default: ./received]: ", theme::tag_prompt());
    io::stdout().flush()?;
    let mut out_dir_str = read_input()?.trim().to_string();
    if out_dir_str.is_empty() {
        out_dir_str = "./received".to_string();
    }

    print!("{} Enable UPnP router port forwarding? (y/N): ", theme::tag_prompt());
    io::stdout().flush()?;
    let upnp_input = read_input()?.trim().to_lowercase();
    let upnp = upnp_input == "y" || upnp_input == "yes";

    print!("{} Shared secret passphrase: ", theme::tag_prompt());
    io::stdout().flush()?;
    let passphrase = read_input()?.trim().to_string();
    if passphrase.is_empty() {
        println!("{} Error: Passphrase cannot be empty.", theme::tag_warn());
        return Ok(());
    }

    println!();
    run_receive_action(Some(&listen), None, upnp, &PathBuf::from(out_dir_str), &passphrase)?;
    Ok(())
}

fn handle_direct_send(theme: &Theme) -> Result<()> {
    println!("{}", theme.border.bold("+--- [2] Send a File (Direct Connection) ---+"));
    print!("{} Path to file to send: ", theme::tag_prompt());
    io::stdout().flush()?;
    let file_str = read_input()?.trim().to_string();
    let file_path = PathBuf::from(&file_str);
    if !file_path.exists() {
        println!("{} Error: File {:?} does not exist.", theme::tag_warn(), file_path);
        return Ok(());
    }

    print!("{} Receiver address (host:port, e.g. 192.168.1.50:9099): ", theme::tag_prompt());
    io::stdout().flush()?;
    let to = read_input()?.trim().to_string();
    if to.is_empty() {
        println!("{} Error: Target address cannot be empty.", theme::tag_warn());
        return Ok(());
    }

    print!("{} Shared secret passphrase: ", theme::tag_prompt());
    io::stdout().flush()?;
    let passphrase = read_input()?.trim().to_string();
    if passphrase.is_empty() {
        println!("{} Error: Passphrase cannot be empty.", theme::tag_warn());
        return Ok(());
    }

    println!();
    run_send_action(&file_path, Some(&to), None, &passphrase, 1048576, false)?;
    Ok(())
}

fn handle_relay_receive(theme: &Theme) -> Result<()> {
    println!("{}", theme.border.bold("+--- [3] Receive via Remote Relay ---+"));
    print!("{} Relay server address [e.g. relay.example.com:9099]: ", theme::tag_prompt());
    io::stdout().flush()?;
    let relay = read_input()?.trim().to_string();
    if relay.is_empty() {
        println!("{} Error: Relay address cannot be empty.", theme::tag_warn());
        return Ok(());
    }

    print!("{} Save files into directory [default: ./received]: ", theme::tag_prompt());
    io::stdout().flush()?;
    let mut out_dir_str = read_input()?.trim().to_string();
    if out_dir_str.is_empty() {
        out_dir_str = "./received".to_string();
    }

    print!("{} Shared secret passphrase: ", theme::tag_prompt());
    io::stdout().flush()?;
    let passphrase = read_input()?.trim().to_string();
    if passphrase.is_empty() {
        println!("{} Error: Passphrase cannot be empty.", theme::tag_warn());
        return Ok(());
    }

    println!();
    run_receive_action(None, Some(&relay), false, &PathBuf::from(out_dir_str), &passphrase)?;
    Ok(())
}

fn handle_relay_send(theme: &Theme) -> Result<()> {
    println!("{}", theme.border.bold("+--- [4] Send via Remote Relay ---+"));
    print!("{} Path to file to send: ", theme::tag_prompt());
    io::stdout().flush()?;
    let file_str = read_input()?.trim().to_string();
    let file_path = PathBuf::from(&file_str);
    if !file_path.exists() {
        println!("{} Error: File {:?} does not exist.", theme::tag_warn(), file_path);
        return Ok(());
    }

    print!("{} Relay server address [e.g. relay.example.com:9099]: ", theme::tag_prompt());
    io::stdout().flush()?;
    let relay = read_input()?.trim().to_string();
    if relay.is_empty() {
        println!("{} Error: Relay address cannot be empty.", theme::tag_warn());
        return Ok(());
    }

    print!("{} Shared secret passphrase: ", theme::tag_prompt());
    io::stdout().flush()?;
    let passphrase = read_input()?.trim().to_string();
    if passphrase.is_empty() {
        println!("{} Error: Passphrase cannot be empty.", theme::tag_warn());
        return Ok(());
    }

    println!();
    run_send_action(&file_path, None, Some(&relay), &passphrase, 1048576, false)?;
    Ok(())
}

fn handle_start_relay(theme: &Theme) -> Result<()> {
    println!("{}", theme.border.bold("+--- [5] Start a Relay Server ---+"));
    print!("{} Relay bind address [default: 0.0.0.0:9099]: ", theme::tag_prompt());
    io::stdout().flush()?;
    let mut listen = read_input()?.trim().to_string();
    if listen.is_empty() {
        listen = "0.0.0.0:9099".to_string();
    }

    println!();
    crate::relay::run_relay(&listen)?;
    Ok(())
}

fn run_self_test(theme: &Theme) -> Result<()> {
    println!("{} Running Cryptographic & Compression Diagnostic...", theme::tag_info());

    let test_data = b"SmartXfer high-entropy validation block for resilience and AEAD verification!";
    let passphrase = "diagnostic_secret_key_123";

    print!("  {} Deriving AEAD key via BLAKE3... ", theme.accent.bold("[1/4]"));
    let key = crypto::derive_key(passphrase)?;
    println!("{}", theme::tag_ok());

    print!("  {} Compressing data with zstd level 3... ", theme.accent.bold("[2/4]"));
    let compressed = zstd::encode_all(&test_data[..], 3)?;
    println!("{} ({} -> {} bytes)", theme::tag_ok(), test_data.len(), compressed.len());

    print!("  {} Encrypting with Orion XChaCha20-Poly1305 AEAD... ", theme.accent.bold("[3/4]"));
    let ciphertext = crypto::encrypt_chunk(&compressed, &key)?;
    println!("{} (ciphertext size: {} bytes)", theme::tag_ok(), ciphertext.len());

    print!("  {} Decrypting, decompressing, and verifying roundtrip... ", theme.accent.bold("[4/4]"));
    let decrypted = crypto::decrypt_chunk(&ciphertext, &key)?;
    let decompressed = zstd::decode_all(decrypted.as_slice())?;
    assert_eq!(&decompressed, test_data);
    println!("{} ({})", theme::tag_ok(), theme.c1.bold("100% Match"));

    println!();
    println!("{} All cryptographic subsystems are operating with 100% integrity.", theme::tag_success());
    Ok(())
}

fn read_input() -> Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input)
}
