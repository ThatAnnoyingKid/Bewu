use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use libssh_rs::AuthMethods;
use libssh_rs::AuthStatus;
use libssh_rs::KnownHosts;
use libssh_rs::PublicKeyHashType;
use libssh_rs::Session;
use libssh_rs::SshOption;

fn verify_known_hosts(session: &Session) -> anyhow::Result<()> {
    let key = session
        .get_server_public_key()?
        .get_public_key_hash_hexa(PublicKeyHashType::Sha256)?;

    match session.is_known_server()? {
        KnownHosts::Ok => {}
        KnownHosts::NotFound | KnownHosts::Unknown => {
            bail!("the server host key is not known. Key: \"{key}\"");
        }
        KnownHosts::Changed => {
            bail!("the server host has changed its key. New Key: \"{key}\"")
        }
        KnownHosts::Other => {
            bail!("the server host has changed its key type. New Key: \"{key}\"")
        }
    }

    Ok(())
}

fn authenticate(session: &Session) -> anyhow::Result<()> {
    if session.userauth_none(None)? == AuthStatus::Success {
        return Ok(());
    }

    let auth_methods = session.userauth_list(None)?;

    ensure!(
        auth_methods.contains(AuthMethods::PUBLIC_KEY),
        "server does not support public key authentication"
    );

    match session.userauth_public_key_auto(None, None)? {
        AuthStatus::Success => {}
        AuthStatus::Denied => {
            bail!("authentication denied");
        }
        AuthStatus::Partial => {
            bail!("partial authentication");
        }
        AuthStatus::Info => {
            bail!("unsupported authentication status");
        }
        AuthStatus::Again => {
            bail!("session should not be in non-blocking mode");
        }
    }

    Ok(())
}

pub fn init_ssh_session(hostname: &str) -> anyhow::Result<Session> {
    println!("Connecting to \"{hostname}\"...");

    let session = Session::new().context("failed to create ssh session")?;
    // session.set_option(SshOption::LogLevel(libssh_rs::LogLevel::Packet))?;
    session.set_option(SshOption::Hostname(hostname.to_string()))?;
    session.options_parse_config(None)?;
    session.connect()?;

    verify_known_hosts(&session).context("failed to verify host key")?;

    let user = session.get_user_name()?;
    println!("Authenticating as \"{user}\"...");

    authenticate(&session).context("failed to authenticate")?;

    Ok(session)
}
