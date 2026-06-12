use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

pub fn setup_tls(
    generate_tls_cert: bool,
    tls_cert: Option<String>,
    tls_key: Option<String>,
) -> Result<Option<(PathBuf, PathBuf)>, Box<dyn std::error::Error>> {
    if !generate_tls_cert && tls_cert.is_none() && tls_key.is_none() {
        return Ok(None);
    }

    let (cert_path, key_path) = match (tls_cert, tls_key) {
        (Some(c), Some(k)) => (PathBuf::from(c), PathBuf::from(k)),
        (None, None) => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            let aura_dir = home.join(".aura");
            if !aura_dir.exists() {
                fs::create_dir_all(&aura_dir)?;
            }
            (aura_dir.join("daemon.crt"), aura_dir.join("daemon.key"))
        }
        _ => {
            return Err("Both --tls-cert and --tls-key must be provided together (unless --generate-tls-cert is used with default paths).".into());
        }
    };

    if generate_tls_cert {
        info!("Generating self-signed TLS certificate and private key...");
        let subject_alt_names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
        let cert_key = rcgen::generate_simple_self_signed(subject_alt_names)?;
        let cert_pem = cert_key.cert.pem();
        let key_pem = cert_key.signing_key.serialize_pem();

        write_private_file(&cert_path, cert_pem.as_bytes())?;
        write_private_file(&key_path, key_pem.as_bytes())?;
        info!("Saved TLS certificate to {:?}", cert_path);
        info!("Saved TLS private key to {:?}", key_path);
    } else {
        if !cert_path.exists() {
            return Err(format!("TLS certificate file does not exist: {:?}", cert_path).into());
        }
        if !key_path.exists() {
            return Err(format!("TLS private key file does not exist: {:?}", key_path).into());
        }
    }

    Ok(Some((cert_path, key_path)))
}

fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        use std::io::Write;
        file.write_all(contents)?;
    }
    #[cfg(not(unix))]
    {
        fs::write(path, contents)?;
    }
    Ok(())
}
