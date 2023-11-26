use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use libc::O_CREAT;
use libc::O_EXCL;
use libc::O_WRONLY;
use libssh_rs::AuthMethods;
use libssh_rs::AuthStatus;
use libssh_rs::KnownHosts;
use libssh_rs::PublicKeyHashType;
use libssh_rs::Session;
use libssh_rs::SshOption;
use rand::distributions::DistString;
use std::fs::File;
use std::io::Write;

#[derive(Debug, argh::FromArgs)]
#[argh(description = "a tool to deploy a deb to a server over")]
pub struct Options {
    #[argh(positional)]
    pub deb_path: Utf8PathBuf,

    #[argh(positional)]
    pub hostname: String,
}

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

fn generate_remote_deb_file_name(input: &Utf8Path) -> anyhow::Result<String> {
    let mut file_stem = input.file_stem().context("missing file stem")?.to_owned();
    let file_extension = input.extension().context("missing file extension")?;

    // Push RNG string to randomize tmp file
    let mut file_stem_extension = String::from("-");
    rand::distributions::Alphanumeric.append_string(
        &mut rand::thread_rng(),
        &mut file_stem_extension,
        10,
    );
    file_stem_extension.push_str("-tmp");
    file_stem.push_str(&file_stem_extension);

    // Push extension
    file_stem.push('.');
    file_stem.push_str(file_extension);

    Ok(file_stem)
}

fn install_package(session: &Session, remote_deb_path: &Utf8Path) -> anyhow::Result<()> {
    let channel = session.new_channel()?;
    channel.open_session()?;
    let command = format!("DEBIAN_FRONTEND=noninteractive sudo apt-get -y --fix-broken reinstall -o DPkg::options::=\"--force-confdef\" -o DPkg::options::=\"--force-confold\" {remote_deb_path} 2>&1");
    channel.request_exec(&command)?;
    channel.send_eof()?;

    {
        let mut stdout_lock = std::io::stdout();
        std::io::copy(&mut channel.stdout(), &mut stdout_lock)?;
    }

    let exit_status = channel.get_exit_status().context("missing exit status")?;
    ensure!(exit_status == 0, "invalid exit status {exit_status}");

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();

    println!("Connecting to \"{}\"...", &options.hostname);

    let session = Session::new().context("failed to create ssh session")?;
    // session.set_option(SshOption::LogLevel(libssh_rs::LogLevel::Packet))?;
    session.set_option(SshOption::Hostname(options.hostname))?;
    session.options_parse_config(None)?;
    session.connect()?;

    verify_known_hosts(&session).context("failed to verify host key")?;

    let user = session.get_user_name()?;
    println!("Authenticating as \"{user}\"...");

    authenticate(&session).context("failed to authenticate")?;

    println!("Opening SFTP channel...");
    let sftp = session.sftp().context("failed to open sftp channel")?;

    println!("Uploading deb package...");
    let local_deb_path = options.deb_path;
    let local_deb_file = File::open(&local_deb_path)
        .with_context(|| format!("failed to open \"{local_deb_path}\""))?;
    let local_deb_file_metadata = local_deb_file.metadata()?;

    let remote_deb_file_name = generate_remote_deb_file_name(&local_deb_path)?;
    // https://refspecs.linuxfoundation.org/FHS_3.0/fhs-3.0.pdf
    let remote_deb_path = Utf8PathBuf::from(format!("/tmp/{remote_deb_file_name}"));
    let mut remote_deb_file = sftp
        .open(
            remote_deb_path.as_str(),
            O_WRONLY | O_CREAT | O_EXCL,
            0o600, // Prevent users from tampering with the file.
        )
        .with_context(|| format!("failed to open \"{remote_deb_path}\" on the server"))?;

    // Perform copy
    let metadata_len = local_deb_file_metadata.len();
    let progress_bar = indicatif::ProgressBar::new(metadata_len);
    let progress_bar_style_template = "[Time = {elapsed_precise} | ETA = {eta_precise} | Speed = {bytes_per_sec}] {wide_bar} {bytes}/{total_bytes}";
    let progress_bar_style = indicatif::ProgressStyle::default_bar()
        .template(progress_bar_style_template)
        .expect("invalid progress bar style template");
    progress_bar.set_style(progress_bar_style);

    let bytes_copied = std::io::copy(
        &mut progress_bar.wrap_read(local_deb_file),
        &mut remote_deb_file,
    )?;
    progress_bar.finish();
    ensure!(
        metadata_len == bytes_copied,
        "file length changed during transfer, (expected) {metadata_len} != (actual) {bytes_copied}",
    );
    remote_deb_file.flush()?;

    println!("Installing...");
    println!();
    let install_result =
        install_package(&session, &remote_deb_path).context("failed to install package");

    println!("Deleting temp file...");
    sftp.remove_file(remote_deb_path.as_str())?;

    install_result
}
