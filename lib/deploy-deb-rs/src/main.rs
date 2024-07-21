use anyhow::ensure;
use anyhow::Context;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use libssh_rs::OpenFlags;
use libssh_rs::Session;
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
    let sudo_password = rpassword::prompt_password("Enter your sudo password: ")?;

    let channel = session.new_channel()?;
    channel.open_session()?;
    let command = format!("DEBIAN_FRONTEND=noninteractive echo {sudo_password} | sudo -S -k -- apt-get -y --fix-broken reinstall -o DPkg::options::=\"--force-confdef\" -o DPkg::options::=\"--force-confold\" {remote_deb_path} 2>&1");
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

    let session = deploy_deb::init_ssh_session(&options.hostname)?;

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
            OpenFlags::WRITE_ONLY | OpenFlags::CREATE_NEW,
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
