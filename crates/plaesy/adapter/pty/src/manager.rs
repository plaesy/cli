use std::io::{Read, Write};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use portable_pty::{native_pty_system, CommandBuilder, PtyPair};

use crate::error::PtyError;
use crate::output::{OutputBuffer, OutputChunk};
use crate::spec::{CommandSpec, TermSize};
use crate::table::{SessionId, SessionTable};

/// Batas atas jumlah sesi terbuka dalam satu manager.
pub const MAX_SESSIONS_LIMIT: usize = 64;
/// Batas atas buffer keluaran per sesi (64 MiB).
pub const MAX_OUTPUT_BUFFER_BYTES: usize = 64 * 1024 * 1024;

const READ_CHUNK_BYTES: usize = 4096;

/// Batas sumber daya untuk `PtyManager`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtyConfig {
    /// Jumlah sesi terbuka maksimum, 1..=`MAX_SESSIONS_LIMIT`.
    pub max_sessions: usize,
    /// Ukuran buffer keluaran per sesi dalam byte, 1..=`MAX_OUTPUT_BUFFER_BYTES`.
    pub output_buffer_bytes: usize,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            max_sessions: 8,
            output_buffer_bytes: 1024 * 1024,
        }
    }
}

impl PtyConfig {
    pub fn validate(&self) -> Result<(), PtyError> {
        if !(1..=MAX_SESSIONS_LIMIT).contains(&self.max_sessions) {
            return Err(PtyError::InvalidConfig("max_sessions is out of range"));
        }
        if !(1..=MAX_OUTPUT_BUFFER_BYTES).contains(&self.output_buffer_bytes) {
            return Err(PtyError::InvalidConfig(
                "output_buffer_bytes is out of range",
            ));
        }
        Ok(())
    }
}

/// Keadaan proses dalam sesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStatus {
    Running,
    Exited { exit_code: u32 },
}

struct Session {
    master: Box<dyn portable_pty::MasterPty + Send>,
    /// Dibungkus `Arc<Mutex<..>>` agar penulisan yang memblokir tidak memegang
    /// kunci tabel sesi.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    output: Arc<Mutex<OutputBuffer>>,
}

/// Mengelola sesi terminal lokal. Aman dipakai dari beberapa thread.
pub struct PtyManager {
    config: PtyConfig,
    sessions: Mutex<SessionTable<Session>>,
}

impl PtyManager {
    pub fn new(config: PtyConfig) -> Result<Self, PtyError> {
        config.validate()?;
        Ok(Self {
            sessions: Mutex::new(SessionTable::new(config.max_sessions)),
            config,
        })
    }

    /// Membuka sesi baru yang menjalankan `command` dengan ukuran layar `size`.
    /// Argumen divalidasi sebelum proses dibuat.
    pub fn open(&self, command: &CommandSpec, size: TermSize) -> Result<SessionId, PtyError> {
        command.validate()?;
        size.validate()?;
        let mut table = self.lock()?;
        if !table.has_capacity() {
            return Err(PtyError::SessionLimitReached {
                limit: self.config.max_sessions,
            });
        }
        // Kunci dipegang selama spawn agar dua pembukaan paralel tidak
        // melampaui batas sesi.
        let session = spawn_session(command, size, self.config.output_buffer_bytes)?;
        table.insert(session)
    }

    /// Menulis `input` ke stdin sesi lalu mengosongkan buffer tulis.
    /// Kunci tabel dilepas sebelum menulis, sehingga pty yang penuh tidak
    /// menahan sesi lain (termasuk `close`).
    pub fn write(&self, id: SessionId, input: &[u8]) -> Result<(), PtyError> {
        let writer = {
            let mut table = self.lock()?;
            let session = table.get_mut(id)?;
            Arc::clone(&session.writer)
        };
        let mut guard = writer.lock().map_err(|_| PtyError::Poisoned)?;
        guard.write_all(input)?;
        guard.flush()?;
        Ok(())
    }

    /// Mengambil keluaran yang belum dibaca. Tidak menunggu keluaran baru.
    pub fn read_output(&self, id: SessionId) -> Result<OutputChunk, PtyError> {
        let output = {
            let mut table = self.lock()?;
            let session = table.get_mut(id)?;
            Arc::clone(&session.output)
        };
        let mut buffer = output.lock().map_err(|_| PtyError::Poisoned)?;
        Ok(buffer.take())
    }

    /// Mengubah ukuran layar sesi dan memberi tahu proses di dalamnya.
    pub fn resize(&self, id: SessionId, size: TermSize) -> Result<(), PtyError> {
        size.validate()?;
        let mut table = self.lock()?;
        let session = table.get_mut(id)?;
        session
            .master
            .resize(size.to_pty_size())
            .map_err(|err| backend("resize pty", err))
    }

    pub fn status(&self, id: SessionId) -> Result<SessionStatus, PtyError> {
        let mut table = self.lock()?;
        let session = table.get_mut(id)?;
        Ok(match session.child.try_wait()? {
            Some(status) => SessionStatus::Exited {
                exit_code: status.exit_code(),
            },
            None => SessionStatus::Running,
        })
    }

    /// Menutup sesi: menghentikan proses bila masih berjalan lalu melepas pty.
    /// Sesi yang sudah dihapus dari tabel tidak lagi dapat diakses.
    pub fn close(&self, id: SessionId) -> Result<(), PtyError> {
        // Sesi dikeluarkan dari tabel sebelum kill dan wait, sehingga kunci
        // tabel tidak ditahan selama itu.
        let mut session = self.lock()?.remove(id)?;
        if let Ok(None) = session.child.try_wait() {
            if session.child.kill().is_ok() {
                // Setelah kill berhasil proses sudah mati (portable-pty
                // mengirim SIGKILL sebagai cadangan); reap agar tidak menjadi zombie.
                let _ = session.child.wait();
            }
        }
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, SessionTable<Session>>, PtyError> {
        self.sessions.lock().map_err(|_| PtyError::Poisoned)
    }
}

fn spawn_session(
    command: &CommandSpec,
    size: TermSize,
    buffer_bytes: usize,
) -> Result<Session, PtyError> {
    let PtyPair { slave, master } = native_pty_system()
        .openpty(size.to_pty_size())
        .map_err(|err| backend("open pty", err))?;

    let mut builder = CommandBuilder::new(&command.program);
    builder.args(&command.args);
    if let Some(cwd) = &command.cwd {
        builder.cwd(cwd);
    }
    let mut child = slave
        .spawn_command(builder)
        .map_err(|err| backend("spawn command", err))?;
    // Slave dilepas di sisi induk agar pembacaan menerima EOF setelah proses keluar.
    drop(slave);

    let output = Arc::new(Mutex::new(OutputBuffer::new(buffer_bytes)));
    let writer = match attach(&*master, Arc::clone(&output)) {
        Ok(writer) => Arc::new(Mutex::new(writer)),
        Err(err) => {
            let _ = child.kill();
            return Err(err);
        }
    };
    Ok(Session {
        master,
        writer,
        child,
        output,
    })
}

/// Mengambil pembaca dan penulis dari master, lalu menjalankan thread pembaca.
fn attach(
    master: &dyn portable_pty::MasterPty,
    output: Arc<Mutex<OutputBuffer>>,
) -> Result<Box<dyn Write + Send>, PtyError> {
    let reader = master
        .try_clone_reader()
        .map_err(|err| backend("clone pty reader", err))?;
    let writer = master
        .take_writer()
        .map_err(|err| backend("take pty writer", err))?;
    spawn_reader(reader, output)?;
    Ok(writer)
}

/// Thread latar yang memindahkan keluaran pty ke buffer. Thread tidak di-join:
/// ia berakhir sendiri ketika pty ditutup dan pembacaan gagal atau mencapai EOF.
fn spawn_reader(
    mut reader: Box<dyn Read + Send>,
    output: Arc<Mutex<OutputBuffer>>,
) -> Result<(), PtyError> {
    let _reader_thread = thread::Builder::new()
        .name("plaesy-pty-reader".to_owned())
        .spawn(move || {
            let mut chunk = [0u8; READ_CHUNK_BYTES];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => {
                        let Ok(mut buffer) = output.lock() else {
                            break;
                        };
                        buffer.push(&chunk[..n]);
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            if let Ok(mut buffer) = output.lock() {
                buffer.mark_closed();
            }
        })?;
    Ok(())
}

fn backend(operation: &'static str, error: impl std::fmt::Display) -> PtyError {
    PtyError::Backend {
        operation,
        message: error.to_string(),
    }
}

#[cfg(test)]
#[path = "manager_test.rs"]
mod manager_test;
