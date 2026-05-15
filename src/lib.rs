use std::env;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process;

use chrono::{Duration, Local, NaiveDateTime};
use flate2::write::GzEncoder;
use flate2::Compression;

const ARCHIVE_DIR: &str = "_archive";
const RETENTION_DAYS: i64 = 14;
const TIMESTAMP_FMT: &str = "%Y_%m_%d_%H_%M";

pub fn main() {
    if let Err(e) = run() {
        eprintln!("auto-archive: {e}");
        process::exit(1);
    }
}

pub fn run() -> io::Result<()> {
    let dir = match env::args().nth(1) {
        Some(arg) => PathBuf::from(arg),
        None => env::current_dir()?,
    };

    if !dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("not a directory: {}", dir.display()),
        ));
    }

    let archive_dir = dir.join(ARCHIVE_DIR);
    fs::create_dir_all(&archive_dir)?;

    let entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .filter(|e| e.file_name() != ARCHIVE_DIR)
        .collect();

    if entries.is_empty() {
        prune_old_archives(&archive_dir)?;
        return Ok(());
    }

    let timestamp = Local::now().format(TIMESTAMP_FMT).to_string();
    let archive_path = archive_dir.join(format!("{timestamp}.tar.gz"));

    write_archive(&archive_path, &entries)?;
    remove_entries(&entries)?;
    prune_old_archives(&archive_dir)?;

    Ok(())
}

fn write_archive(archive_path: &Path, entries: &[fs::DirEntry]) -> io::Result<()> {
    let file = File::create(archive_path)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut builder = tar::Builder::new(encoder);

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            builder.append_dir_all(&name, &path)?;
        } else {
            builder.append_path_with_name(&path, &name)?;
        }
    }

    builder.into_inner()?.finish()?;
    Ok(())
}

fn remove_entries(entries: &[fs::DirEntry]) -> io::Result<()> {
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn prune_old_archives(archive_dir: &Path) -> io::Result<()> {
    let cutoff = Local::now().naive_local() - Duration::days(RETENTION_DAYS);

    for entry in fs::read_dir(archive_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else { continue };
        let Some(stem) = name_str.strip_suffix(".tar.gz") else { continue };
        let Ok(dt) = NaiveDateTime::parse_from_str(stem, TIMESTAMP_FMT) else { continue };

        if dt < cutoff {
            fs::remove_file(entry.path())?;
        }
    }

    Ok(())
}
