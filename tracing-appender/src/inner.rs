use std::io::{BufWriter, Write};
use std::{fs, io};

use crate::rolling::Rotation;
use chrono::prelude::*;
use std::fmt::Debug;
use std::fs::{File, OpenOptions};
use std::path::Path;


pub struct CustomFormatter(pub fn(&DateTime<Utc>) -> String);

impl Debug for CustomFormatter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CustomFormatter")
    }
}

pub trait LogFileFormatter {
    fn format(&self, rotation: &Rotation, date: &DateTime<Utc>) -> String;
}

impl<T> LogFileFormatter for T 
    where T: AsRef<Path> 
{
    fn format(&self, rotation: &Rotation, date: &DateTime<Utc>) -> String {
        rotation.join_date(self.as_ref().to_str().unwrap(), date)
    }
}

impl LogFileFormatter for CustomFormatter {
    fn format(&self, _: &Rotation, date: &DateTime<Utc>) -> String {
        self.0(date)
    }
}

#[derive(Debug)]
pub(crate) struct InnerAppender<F: LogFileFormatter> {
    log_directory: String,
    log_filename_formatter: F,
    writer: BufWriter<File>,
    next_date: DateTime<Utc>,
    rotation: Rotation,
}

impl<F: LogFileFormatter> io::Write for InnerAppender<F> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let now = Utc::now();
        self.write_timestamped(buf, now)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<F: LogFileFormatter> InnerAppender<F> {
    pub(crate) fn new(
        log_directory: &Path,
        log_filename_formatter: F,
        rotation: Rotation,
        now: DateTime<Utc>,
    ) -> io::Result<Self> {
        let log_directory = log_directory.to_str().unwrap();

        let filename = log_filename_formatter.format(&rotation, &now);
        let next_date = rotation.next_date(&now);

        Ok(InnerAppender {
            log_directory: log_directory.to_string(),
            log_filename_formatter,
            writer: create_writer(log_directory, &filename)?,
            next_date,
            rotation,
        })
    }

    fn write_timestamped(&mut self, buf: &[u8], date: DateTime<Utc>) -> io::Result<usize> {
        // Even if refresh_writer fails, we still have the original writer. Ignore errors
        // and proceed with the write.
        let buf_len = buf.len();
        self.refresh_writer(date);
        self.writer.write_all(buf).map(|_| buf_len)
    }

    fn refresh_writer(&mut self, now: DateTime<Utc>) {
        if self.should_rollover(now) {
            let filename = self.log_filename_formatter.format(&self.rotation, &now);

            self.next_date = self.rotation.next_date(&now);

            match create_writer(&self.log_directory, &filename) {
                Ok(writer) => self.writer = writer,
                Err(err) => eprintln!("Couldn't create writer for logs: {}", err),
            }
        }
    }

    fn should_rollover(&self, date: DateTime<Utc>) -> bool {
        date >= self.next_date
    }
}

fn create_writer(directory: &str, filename: &str) -> io::Result<BufWriter<File>> {
    let file_path = Path::new(directory).join(filename);
    Ok(BufWriter::new(open_file_create_parent_dirs(&file_path)?))
}

fn open_file_create_parent_dirs(path: &Path) -> io::Result<File> {
    let mut open_options = OpenOptions::new();
    open_options.append(true).create(true);

    let new_file = open_options.open(path);
    if new_file.is_err() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            return open_options.open(path);
        }
    }

    new_file
}
