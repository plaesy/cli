//! Sesi pengguna: token dan identitas. Tidak menyimpan kata sandi.

/// Identitas pengguna yang sedang masuk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: String,
    pub name: String,
    pub roles: Vec<String>,
}

/// Keadaan sesi. Kosong bila belum masuk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    token: Option<String>,
    user: Option<User>,
}

impl Session {
    /// Sesi kosong (belum masuk).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Menyimpan token dan pengguna. Token kosong ditolak dan sesi tidak berubah.
    ///
    /// # Errors
    ///
    /// Galat bila token kosong.
    pub fn sign_in(&mut self, token: &str, user: User) -> Result<(), SessionError> {
        if token.trim().is_empty() {
            return Err(SessionError::EmptyToken);
        }
        self.token = Some(token.to_owned());
        self.user = Some(user);
        Ok(())
    }

    /// Menghapus token dan pengguna.
    pub fn sign_out(&mut self) {
        self.token = None;
        self.user = None;
    }

    /// Benar bila token tersimpan.
    #[must_use]
    pub fn is_signed_in(&self) -> bool {
        self.token.is_some()
    }

    /// Token yang tersimpan, bila ada.
    #[must_use]
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Pengguna yang sedang masuk, bila ada.
    #[must_use]
    pub fn user(&self) -> Option<&User> {
        self.user.as_ref()
    }

    /// Benar bila pengguna memiliki peran `role`.
    #[must_use]
    pub fn has_role(&self, role: &str) -> bool {
        self.user
            .as_ref()
            .is_some_and(|u| u.roles.iter().any(|r| r == role))
    }
}

/// Galat sesi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    EmptyToken,
}

#[cfg(test)]
#[path = "session_test.rs"]
mod session_test;
