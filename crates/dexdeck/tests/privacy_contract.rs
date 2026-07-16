use std::{fs, path::Path};

const FORBIDDEN_CRATES: &[&str] = &[
    "reqwest",
    "hyper",
    "curl",
    "ureq",
    "rustls",
    "native-tls",
    "openssl",
    "tonic",
    "tungstenite",
    "sentry",
    "opentelemetry",
];

const FORBIDDEN_RUNTIME_SYMBOLS: &[&str] = &[
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "tokio::net",
    "std::net",
    "reqwest::",
    "hyper::",
    "sentry::",
    "opentelemetry::",
];

#[test]
fn product_dependency_tree_has_no_network_or_telemetry_stack()
-> Result<(), Box<dyn std::error::Error>> {
    let root = workspace_root();
    let lock = fs::read_to_string(root.join("Cargo.lock"))?;
    for dependency in FORBIDDEN_CRATES {
        assert!(
            !lock.contains(&format!("name = \"{dependency}\"")),
            "forbidden runtime-capable dependency: {dependency}"
        );
    }
    Ok(())
}

#[test]
fn product_source_has_no_socket_or_telemetry_calls() -> Result<(), Box<dyn std::error::Error>> {
    let crates = workspace_root().join("crates");
    visit_rust(&crates, &mut |path, source| {
        if path.ends_with("privacy_contract.rs") {
            return;
        }
        for symbol in FORBIDDEN_RUNTIME_SYMBOLS {
            assert!(
                !source.contains(symbol),
                "forbidden runtime symbol {symbol} in {}",
                path.display()
            );
        }
    })?;
    Ok(())
}

#[test]
fn product_never_invokes_a_command_shell() -> Result<(), Box<dyn std::error::Error>> {
    let crates = workspace_root().join("crates");
    visit_rust(&crates, &mut |path, source| {
        if path.ends_with("privacy_contract.rs") {
            return;
        }
        for shell in [
            "Command::new(\"sh\")",
            "Command::new(\"bash\")",
            "cmd.exe /C",
        ] {
            assert!(
                !source.contains(shell),
                "shell invocation {shell} in {}",
                path.display()
            );
        }
    })?;
    Ok(())
}

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| panic!("workspace root must contain crates/dexdeck"))
        .to_path_buf()
}

fn visit_rust(
    directory: &Path,
    visitor: &mut dyn FnMut(&Path, &str),
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            visit_rust(&path, visitor)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path)?;
            visitor(&path, &source);
        }
    }
    Ok(())
}
