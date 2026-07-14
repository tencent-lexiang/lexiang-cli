use crate::config::Config;
use crate::mcp::upload::{guess_content_type, semantic_extension};
use crate::mcp::{McpClient, UploadConfig};
use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::output::{print_output, FieldFilter};

const MAX_HTML_UPLOAD_SIZE: u64 = 10 * 1024 * 1024;
const OUTPUT_FORMATS: [&str; 6] = ["json", "json-pretty", "table", "yaml", "csv", "markdown"];

#[derive(Debug, PartialEq, Eq)]
struct ResolvedUploadMetadata {
    file_name: String,
    mime_type: String,
    extension: Option<String>,
}

#[derive(Debug)]
struct PreparedHtmlBundle {
    archive: NamedTempFile,
    remote_name: String,
    file_count: usize,
}

/// 处理需要读取本地文件的增强命令。
///
/// 只认领 `file upload`、`entry import` 及两个 namespace 的帮助；
/// 其他命令返回 false，继续走动态 MCP 命令分发。
pub async fn try_handle_local_file_command(args: &[String]) -> Result<bool> {
    if args.len() < 3 {
        return Ok(false);
    }

    let namespace = args[1].as_str();
    let subcommand = args[2].as_str();
    let claimed = matches!(
        (namespace, subcommand),
        ("file", "upload") | ("entry", "import") | ("file" | "entry", "--help" | "-h")
    );
    if !claimed {
        return Ok(false);
    }

    let matches = match build_command().try_get_matches_from(args) {
        Ok(matches) => matches,
        Err(error) => {
            error.print().ok();
            if error.use_stderr() {
                return Err(error.into());
            }
            return Ok(true);
        }
    };

    match matches.subcommand() {
        Some(("file", file_matches)) => {
            if let Some(("upload", upload_matches)) = file_matches.subcommand() {
                handle_file_upload(upload_matches).await?;
            }
        }
        Some(("entry", entry_matches)) => {
            if let Some(("import", import_matches)) = entry_matches.subcommand() {
                match import_matches.subcommand() {
                    Some(("markdown", format_matches)) => {
                        handle_entry_import(format_matches, "markdown", false).await?;
                    }
                    Some(("html", format_matches)) => {
                        handle_entry_import(format_matches, "html", format_matches.get_flag("dir"))
                            .await?;
                    }
                    _ => return Ok(false),
                }
            }
        }
        _ => return Ok(false),
    }

    Ok(true)
}

fn build_command() -> Command {
    Command::new("lx")
        .subcommand_required(true)
        .subcommand(
            Command::new("file")
                .about("File operations (local upload + dynamic MCP commands)")
                .after_long_help(
                    "Other file commands are generated from MCP schema.\n\
                     Run `lx tools list --category file` to inspect them.",
                )
                .subcommand(file_upload_command()),
        )
        .subcommand(
            Command::new("entry")
                .about("Entry operations (local import + dynamic MCP commands)")
                .after_long_help(
                    "Other entry commands are generated from MCP schema.\n\
                     Run `lx tools list --category entry` to inspect them.",
                )
                .subcommand(entry_import_command()),
        )
}

fn file_upload_command() -> Command {
    Command::new("upload")
        .about("Upload a local file through apply, HTTP PUT, and commit")
        .arg(Arg::new("path").required(true).value_name("PATH"))
        .arg(
            Arg::new("parent_entry_id")
                .long("parent-entry-id")
                .required(true)
                .value_name("ENTRY_ID")
                .help("Parent entry ID for a new file; current file entry ID for an update"),
        )
        .arg(
            Arg::new("file_id")
                .long("file-id")
                .value_name("FILE_ID")
                .help("Existing file ID when uploading a new revision"),
        )
        .arg(
            Arg::new("name")
                .long("name")
                .value_name("NAME")
                .help("Remote file name (defaults to local file name)"),
        )
        .arg(
            Arg::new("mime_type")
                .long("mime-type")
                .value_name("MIME")
                .help("Physical MIME type override"),
        )
        .arg(
            Arg::new("extension")
                .long("extension")
                .value_name("EXT")
                .help("Semantic entry extension override, without storage semantics"),
        )
        .arg(
            Arg::new("html_bundle")
                .long("html-bundle")
                .help("Treat a ZIP as an HTML bundle (application/zip + extension=html)")
                .action(ArgAction::SetTrue),
        )
        .arg(output_format_arg())
}

fn entry_import_command() -> Command {
    Command::new("import")
        .about("Import local Markdown, HTML, or an HTML material directory")
        .subcommand_required(true)
        .subcommand(entry_import_format_command("markdown", false))
        .subcommand(entry_import_format_command("html", true))
}

fn entry_import_format_command(format: &'static str, supports_directory: bool) -> Command {
    let about = match format {
        "markdown" => "Import a local Markdown file as editable page content",
        "html" => "Import an HTML file as page content or a directory as an HTML bundle",
        _ => unreachable!("known import format"),
    };
    let name_help = if supports_directory {
        "Page title or HTML bundle name (defaults to the local path name)"
    } else {
        "Page title (defaults to the local file stem)"
    };
    let mut command = Command::new(format)
        .about(about)
        .arg(Arg::new("path").required(true).value_name("PATH"))
        .arg(
            Arg::new("space_id")
                .long("space-id")
                .value_name("SPACE_ID")
                .help("Create a page at the root of this space"),
        )
        .arg(
            Arg::new("parent_id")
                .long("parent-id")
                .value_name("ENTRY_ID")
                .help("Create a page under this parent entry"),
        )
        .arg(
            Arg::new("entry_id")
                .long("entry-id")
                .value_name("ENTRY_ID")
                .help("Import into an existing page"),
        )
        .group(
            ArgGroup::new("destination")
                .args(["space_id", "parent_id", "entry_id"])
                .required(true)
                .multiple(false),
        )
        .arg(
            Arg::new("name")
                .long("name")
                .value_name("TITLE")
                .help(name_help),
        )
        .arg(
            Arg::new("before")
                .long("before")
                .value_name("ENTRY_ID")
                .help("Insert a newly imported page before this entry"),
        )
        .arg(
            Arg::new("after_block_id")
                .long("after-block-id")
                .value_name("BLOCK_ID")
                .help("Append to an existing page after this root-level block"),
        )
        .arg(
            Arg::new("force_write")
                .long("force-write")
                .help("Replace all content in an existing page")
                .action(ArgAction::SetTrue),
        )
        .arg(output_format_arg());

    if supports_directory {
        command = command.arg(
            Arg::new("dir")
                .long("dir")
                .help("Package PATH as an HTML bundle; requires --parent-id")
                .requires("parent_id")
                .conflicts_with_all([
                    "space_id",
                    "entry_id",
                    "before",
                    "after_block_id",
                    "force_write",
                ])
                .action(ArgAction::SetTrue),
        );
    }

    command
}

fn output_format_arg() -> Arg {
    Arg::new("format")
        .short('o')
        .long("format")
        .value_name("FORMAT")
        .default_value("json-pretty")
        .value_parser(OUTPUT_FORMATS)
}

async fn handle_file_upload(matches: &ArgMatches) -> Result<()> {
    let path = PathBuf::from(required_string(matches, "path")?);
    let metadata = std::fs::metadata(&path)
        .with_context(|| format!("Cannot read local file metadata: {}", path.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("Upload path is not a regular file: {}", path.display());
    }
    if metadata.len() == 0 {
        anyhow::bail!("Cannot upload an empty file: {}", path.display());
    }

    let resolved = resolve_upload_metadata(
        &path,
        matches.get_one::<String>("name").map(String::as_str),
        matches.get_one::<String>("mime_type").map(String::as_str),
        matches.get_one::<String>("extension").map(String::as_str),
        matches.get_flag("html_bundle"),
    )?;
    if resolved.extension.as_deref() == Some("html") && metadata.len() > MAX_HTML_UPLOAD_SIZE {
        anyhow::bail!("HTML uploads are limited to 10 MiB by the server");
    }

    let config = Config::load()?;
    let access_token = crate::auth::get_access_token(&config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;
    let upload_config = UploadConfig {
        file_id: matches.get_one::<String>("file_id").cloned(),
        parent_entry_id: crate::cmd::utils::parse_entry_id(required_string(
            matches,
            "parent_entry_id",
        )?),
        file_name: Some(resolved.file_name.clone()),
        content_type: Some(resolved.mime_type.clone()),
        extension: resolved.extension,
    };

    eprintln!("Uploading {} as {}...", path.display(), resolved.mime_type);
    let result = client.upload_file_result(&upload_config, &path).await?;
    print_mcp_result(&result, matches)
}

async fn handle_entry_import(
    matches: &ArgMatches,
    content_type: &'static str,
    directory: bool,
) -> Result<()> {
    let path = PathBuf::from(required_string(matches, "path")?);

    if directory {
        return handle_html_directory_import(matches, &path).await;
    }

    let metadata = std::fs::metadata(&path)
        .with_context(|| format!("Cannot read import source metadata: {}", path.display()))?;
    if !metadata.is_file() {
        anyhow::bail!("Import source is not a regular file: {}", path.display());
    }
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Import source must be valid UTF-8: {}", path.display()))?;
    let entry_id = matches.get_one::<String>("entry_id");

    if entry_id.is_some() {
        if matches.get_one::<String>("name").is_some()
            || matches.get_one::<String>("before").is_some()
        {
            anyhow::bail!("--name and --before are only valid when creating a new page");
        }
    } else if matches.get_one::<String>("after_block_id").is_some()
        || matches.get_flag("force_write")
    {
        anyhow::bail!("--after-block-id and --force-write require --entry-id");
    }

    let (tool_name, args) = if let Some(entry_id) = entry_id {
        let mut args = serde_json::json!({
            "entry_id": crate::cmd::utils::parse_entry_id(entry_id),
            "content": content,
            "content_type": content_type,
        });
        if let Some(after_block_id) = matches.get_one::<String>("after_block_id") {
            args["after_block_id"] = serde_json::json!(after_block_id);
        }
        if matches.get_flag("force_write") {
            args["force_write"] = serde_json::json!(true);
        }
        ("entry_import_content_to_entry", args)
    } else {
        let title = matches
            .get_one::<String>("name")
            .cloned()
            .or_else(|| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
            })
            .filter(|name| !name.is_empty())
            .ok_or_else(|| anyhow::anyhow!("Cannot determine page title"))?;
        let mut args = serde_json::json!({
            "name": title,
            "content": content,
            "content_type": content_type,
        });
        if let Some(space_id) = matches.get_one::<String>("space_id") {
            args["space_id"] = serde_json::json!(crate::cmd::utils::parse_space_id(space_id));
        }
        if let Some(parent_id) = matches.get_one::<String>("parent_id") {
            args["parent_id"] = serde_json::json!(crate::cmd::utils::parse_entry_id(parent_id));
        }
        if let Some(before) = matches.get_one::<String>("before") {
            args["before"] = serde_json::json!(crate::cmd::utils::parse_entry_id(before));
        }
        ("entry_import_content", args)
    };

    let config = Config::load()?;
    let access_token = crate::auth::get_access_token(&config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;
    let result = client.call_tool(tool_name, args).await?;
    print_mcp_result(&result, matches)
}

async fn handle_html_directory_import(matches: &ArgMatches, path: &Path) -> Result<()> {
    let parent_id = matches
        .get_one::<String>("parent_id")
        .ok_or_else(|| anyhow::anyhow!("HTML directory import requires --parent-id"))?;
    if matches.get_one::<String>("space_id").is_some()
        || matches.get_one::<String>("entry_id").is_some()
        || matches.get_one::<String>("before").is_some()
        || matches.get_one::<String>("after_block_id").is_some()
        || matches.get_flag("force_write")
    {
        anyhow::bail!(
            "HTML directory import only supports --parent-id, optional --name, and --format"
        );
    }

    // Local validation and packaging deliberately happen before auth or MCP apply.
    let bundle = prepare_html_bundle(path, matches.get_one::<String>("name").map(String::as_str))?;
    let bundle_size = bundle.archive.as_file().metadata()?.len();

    let config = Config::load()?;
    let access_token = crate::auth::get_access_token(&config).await?;
    let client = McpClient::new(&config.mcp.url, Some(access_token))?;
    let upload_config = html_bundle_upload_config(parent_id, &bundle.remote_name);

    eprintln!(
        "Uploading {} files from {} as HTML bundle {} ({} bytes)...",
        bundle.file_count,
        path.display(),
        bundle.remote_name,
        bundle_size
    );
    let result = client
        .upload_file_result(&upload_config, bundle.archive.path())
        .await?;
    print_mcp_result(&result, matches)
}

fn html_bundle_upload_config(parent_id: &str, remote_name: &str) -> UploadConfig {
    UploadConfig {
        file_id: None,
        parent_entry_id: crate::cmd::utils::parse_entry_id(parent_id),
        file_name: Some(remote_name.to_string()),
        content_type: Some("application/zip".to_string()),
        extension: Some("html".to_string()),
    }
}

fn prepare_html_bundle(path: &Path, name: Option<&str>) -> Result<PreparedHtmlBundle> {
    let root_metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("Cannot read HTML directory: {}", path.display()))?;
    if root_metadata.file_type().is_symlink() {
        anyhow::bail!(
            "HTML directory cannot be a symbolic link: {}",
            path.display()
        );
    }
    if !root_metadata.is_dir() {
        anyhow::bail!("--dir requires a directory: {}", path.display());
    }

    let index_path = path.join("index.html");
    let index_metadata = std::fs::symlink_metadata(&index_path).with_context(|| {
        format!(
            "HTML directory must contain index.html at its root: {}",
            path.display()
        )
    })?;
    if index_metadata.file_type().is_symlink() || !index_metadata.is_file() {
        anyhow::bail!(
            "HTML directory root index.html must be a regular file: {}",
            index_path.display()
        );
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(path).follow_links(false) {
        let entry =
            entry.with_context(|| format!("Cannot walk HTML directory: {}", path.display()))?;
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            anyhow::bail!(
                "HTML directory cannot contain symbolic links: {}",
                entry.path().display()
            );
        }
        if file_type.is_dir() {
            continue;
        }
        if !file_type.is_file() {
            anyhow::bail!(
                "HTML directory contains an unsupported filesystem entry: {}",
                entry.path().display()
            );
        }

        let relative = entry.path().strip_prefix(path).with_context(|| {
            format!(
                "Cannot make path relative to HTML directory: {}",
                entry.path().display()
            )
        })?;
        let archive_name = relative
            .to_str()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "HTML bundle paths must be valid UTF-8: {}",
                    relative.display()
                )
            })?
            .replace(std::path::MAIN_SEPARATOR, "/");
        files.push((archive_name, entry.into_path()));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut archive = NamedTempFile::new().context("Cannot create temporary HTML bundle")?;
    {
        let mut writer = ZipWriter::new(archive.as_file_mut());
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        for (archive_name, source_path) in &files {
            writer
                .start_file(archive_name, options)
                .with_context(|| format!("Cannot add {archive_name} to HTML bundle"))?;
            let mut source = File::open(source_path)
                .with_context(|| format!("Cannot open HTML asset: {}", source_path.display()))?;
            std::io::copy(&mut source, &mut writer).with_context(|| {
                format!("Cannot compress HTML asset: {}", source_path.display())
            })?;
        }
        writer.finish().context("Cannot finish HTML bundle ZIP")?;
    }
    archive.as_file_mut().flush()?;
    validate_html_bundle_size(archive.as_file().metadata()?.len())?;

    let remote_name = html_bundle_name(path, name)?;
    Ok(PreparedHtmlBundle {
        archive,
        remote_name,
        file_count: files.len(),
    })
}

fn html_bundle_name(path: &Path, name: Option<&str>) -> Result<String> {
    let base = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            path.file_name()
                .map(|value| value.to_string_lossy().to_string())
        })
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine HTML bundle name"))?;
    if base.to_ascii_lowercase().ends_with(".zip") {
        Ok(base)
    } else {
        Ok(format!("{base}.zip"))
    }
}

fn validate_html_bundle_size(size: u64) -> Result<()> {
    if size > MAX_HTML_UPLOAD_SIZE {
        anyhow::bail!("HTML bundle exceeds the 10 MiB upload limit after compression");
    }
    Ok(())
}

fn resolve_upload_metadata(
    path: &Path,
    name: Option<&str>,
    mime_type: Option<&str>,
    extension: Option<&str>,
    html_bundle: bool,
) -> Result<ResolvedUploadMetadata> {
    let file_name = name
        .map(str::to_string)
        .or_else(|| {
            path.file_name()
                .map(|value| value.to_string_lossy().to_string())
        })
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Cannot determine file name"))?;
    let inferred_extension = semantic_extension(&file_name);

    if html_bundle {
        if inferred_extension.as_deref() != Some("zip") {
            anyhow::bail!("--html-bundle requires a .zip upload name");
        }
        if let Some(mime_type) = mime_type {
            if mime_type != "application/zip" {
                anyhow::bail!("--html-bundle requires --mime-type application/zip");
            }
        }
        if let Some(extension) = extension {
            if normalize_extension(extension) != "html" {
                anyhow::bail!("--html-bundle cannot be combined with a non-html extension");
            }
        }
        return Ok(ResolvedUploadMetadata {
            file_name,
            mime_type: "application/zip".to_string(),
            extension: Some("html".to_string()),
        });
    }

    Ok(ResolvedUploadMetadata {
        mime_type: mime_type
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| guess_content_type(&file_name)),
        extension: extension
            .map(normalize_extension)
            .filter(|value| !value.is_empty())
            .or(inferred_extension),
        file_name,
    })
}

fn normalize_extension(extension: &str) -> String {
    extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
}

fn print_mcp_result(result: &serde_json::Value, matches: &ArgMatches) -> Result<()> {
    let data = result.get("data").unwrap_or(result);
    let format = required_string(matches, "format")?;
    print_output(data, format, &FieldFilter::new(None, false))
}

fn required_string<'a>(matches: &'a ArgMatches, name: &str) -> Result<&'a str> {
    matches
        .get_one::<String>(name)
        .map(String::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing required argument: {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_html_bundle_without_changing_physical_mime() {
        let resolved =
            resolve_upload_metadata(Path::new("site.zip"), None, None, None, true).unwrap();

        assert_eq!(resolved.file_name, "site.zip");
        assert_eq!(resolved.mime_type, "application/zip");
        assert_eq!(resolved.extension.as_deref(), Some("html"));
    }

    #[test]
    fn ordinary_zip_keeps_zip_semantics() {
        let resolved =
            resolve_upload_metadata(Path::new("archive.zip"), None, None, None, false).unwrap();

        assert_eq!(resolved.mime_type, "application/zip");
        assert_eq!(resolved.extension.as_deref(), Some("zip"));
    }

    #[test]
    fn rejects_non_zip_html_bundle() {
        let error =
            resolve_upload_metadata(Path::new("index.html"), None, None, None, true).unwrap_err();

        assert!(error.to_string().contains("requires a .zip"));
    }

    #[test]
    fn structured_import_parser_selects_format_and_directory_mode() {
        let matches = build_command()
            .try_get_matches_from([
                "lx",
                "entry",
                "import",
                "html",
                "./site",
                "--dir",
                "--parent-id",
                "entry-1",
            ])
            .unwrap();
        let format_matches = matches
            .subcommand_matches("entry")
            .unwrap()
            .subcommand_matches("import")
            .unwrap()
            .subcommand_matches("html")
            .unwrap();

        assert!(format_matches.get_flag("dir"));
        assert_eq!(
            format_matches.get_one::<String>("path").map(String::as_str),
            Some("./site")
        );
    }

    #[test]
    fn packages_html_directory_without_wrapper_folder() {
        use std::io::Read;

        let temp = tempfile::tempdir().unwrap();
        let site = temp.path().join("site");
        std::fs::create_dir_all(site.join("assets")).unwrap();
        std::fs::write(site.join("index.html"), "<h1>Hello</h1>").unwrap();
        std::fs::write(site.join("assets/app.js"), "console.log('ok')").unwrap();

        let bundle = prepare_html_bundle(&site, Some("landing")).unwrap();
        let mut archive = zip::ZipArchive::new(bundle.archive.reopen().unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect();

        assert_eq!(names, ["assets/app.js", "index.html"]);
        assert_eq!(bundle.remote_name, "landing.zip");
        assert_eq!(bundle.file_count, 2);
        let mut index_html = String::new();
        archive
            .by_name("index.html")
            .unwrap()
            .read_to_string(&mut index_html)
            .unwrap();
        assert_eq!(index_html, "<h1>Hello</h1>");
    }

    #[test]
    fn rejects_html_directory_without_root_index() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("page.html"), "<p>not index</p>").unwrap();

        let error = prepare_html_bundle(temp.path(), None).unwrap_err();

        assert!(error.to_string().contains("index.html at its root"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_in_html_directory() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("index.html"), "<h1>Hello</h1>").unwrap();
        std::fs::write(temp.path().join("outside.js"), "alert(1)").unwrap();
        symlink(
            temp.path().join("outside.js"),
            temp.path().join("linked.js"),
        )
        .unwrap();

        let error = prepare_html_bundle(temp.path(), None).unwrap_err();

        assert!(error.to_string().contains("symbolic links"));
    }

    #[test]
    fn rejects_html_bundle_over_server_limit() {
        let error = validate_html_bundle_size(MAX_HTML_UPLOAD_SIZE + 1).unwrap_err();

        assert!(error.to_string().contains("10 MiB"));
    }

    #[test]
    fn html_directory_routes_through_html_bundle_upload_semantics() {
        let config =
            html_bundle_upload_config("https://lexiangla.com/pages/parent-1", "landing.zip");

        assert_eq!(config.parent_entry_id, "parent-1");
        assert_eq!(config.file_name.as_deref(), Some("landing.zip"));
        assert_eq!(config.content_type.as_deref(), Some("application/zip"));
        assert_eq!(config.extension.as_deref(), Some("html"));
        assert!(config.file_id.is_none());
    }

    #[test]
    fn local_command_parser_does_not_claim_dynamic_commands() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handled = runtime
            .block_on(try_handle_local_file_command(&[
                "lx".to_string(),
                "file".to_string(),
                "describe-file".to_string(),
            ]))
            .unwrap();

        assert!(!handled);
    }

    #[test]
    fn removed_import_file_command_is_not_claimed() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handled = runtime
            .block_on(try_handle_local_file_command(&[
                "lx".to_string(),
                "entry".to_string(),
                "import-file".to_string(),
            ]))
            .unwrap();

        assert!(!handled);
    }
}
