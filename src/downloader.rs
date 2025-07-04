use anyhow::{anyhow, Result};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use log::*;
use reqwest::blocking::Client;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use zip::read::ZipFile;
use zip::ZipArchive;

pub fn download_file_with_progress(client: &Client, url: &str, path: &Path) -> Result<()> {
    info!("Downloading from: {url}");

    let mut response = client.get(url).send()?.error_for_status()?;
    let total_size = response.content_length().unwrap_or(0);

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .progress_chars("#>-"));

    let mut file = File::create(path)?;
    let mut downloaded = 0;

    let mut buffer = [0; 8192];
    while let Ok(n) = response.read(&mut buffer) {
        if n == 0 {
            break;
        }
        file.write_all(&buffer[..n])?;
        downloaded += n as u64;
        pb.set_position(downloaded);
    }

    pb.finish_with_message("Downloaded");
    info!("Downloaded to {}", path.display());
    Ok(())
}

pub fn unzip_file(zip_path: &Path, dest_dir: &Path) -> Result<()> {
    info!("Unzipping...");
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        if (*file.name()).ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    info!("Unzipped to {}", dest_dir.display());
    Ok(())
}

pub fn decompress_gz(gz_path: &Path, dest_path: &Path) -> Result<()> {
    info!("Decompressing gz...");
    let mut gz_file = GzDecoder::new(File::open(gz_path)?);
    let mut dest_file = File::create(dest_path)?;
    io::copy(&mut gz_file, &mut dest_file)?;
    info!("Decompressed to {}", dest_path.display());
    Ok(())
}

pub fn decompress_zip(zip_path: &Path, dest_path: &Path) -> Result<()> {
    info!("Decompressing zip...");
    let mut zip_archive = ZipArchive::new(File::open(zip_path)?)?;
    let mut dest_file = File::create(dest_path)?;

    let mut zip_file: ZipFile = match zip_archive.len() {
        1 => zip_archive
            .by_index(0)
            .map_err(|e| anyhow!("Failed to open zip file: {e}")),
        0 => Err(anyhow!("Zip file is empty")),
        n => Err(anyhow!("Zip file contains multiple files ({})", n)),
    }?;

    io::copy(&mut zip_file, &mut dest_file)?;
    info!("Decompressed to {}", dest_path.display());
    Ok(())
}
