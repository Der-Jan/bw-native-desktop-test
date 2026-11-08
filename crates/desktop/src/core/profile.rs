use super::*;
use services::accounts::{
    certificate,
    profile::{Origin, ServerProfile},
    storage::read_bounded,
    transport::Transport,
};
impl Core {
    pub(super) fn restore_profile(&mut self, store: &ProfileStore) {
        match store.selected_login_profile() {
            Ok(Some(profile)) => self.profile = Some(profile),
            Ok(None) => {
                let cloud = ServerProfile::cloud(false);
                let profile = store
                    .list()
                    .into_iter()
                    .find(|p| !p.account_bound && p.base == cloud.base)
                    .map(Ok)
                    .unwrap_or_else(|| store.save(cloud, None));
                match profile.and_then(|p| {
                    store.select_login_profile(p.id)?;
                    Ok(p)
                }) {
                    Ok(p) => self.profile = Some(p),
                    Err(e) => self.profile_error = Some(e),
                }
            }
            Err(error) => {
                self.profile = None;
                self.profile_error = Some(error);
            }
        }
    }
    pub fn select_cloud(&mut self, eu: bool) {
        if self.profile_busy {
            return;
        }
        let Some(store) = self.profiles.clone() else {
            self.ensure_profiles();
            return;
        };
        let cloud = ServerProfile::cloud(eu);
        if let Some(profile) = store
            .list()
            .into_iter()
            .find(|p| !p.account_bound && p.base == cloud.base)
        {
            self.select_profile(profile.id);
            return;
        }
        match store.save(cloud, None) {
            Ok(p) => self.select_profile(p.id),
            Err(e) => self.profile_error = Some(e),
        }
    }
    pub fn ensure_profiles(&mut self) {
        if self.profiles.is_some() || self.profile_busy {
            return;
        }
        self.profile_busy = true;
        let root = crate::paths::data_dir();
        self.events.spawn(
            tokio::task::spawn_blocking(move || ProfileStore::system(&root)),
            |core, result| {
                core.profile_busy = false;
                match result {
                    Ok(Ok(store)) => {
                        core.restore_profile(&store);
                        core.profiles = Some(store);
                    }
                    Ok(Err(e)) => core.profile_error = Some(e),
                    Err(_) => core.profile_error = Some("Server settings unavailable".into()),
                }
            },
        );
    }
    pub fn select_profile(&mut self, id: uuid::Uuid) {
        if self.profile_busy {
            return;
        }
        let Some(store) = &self.profiles else {
            return;
        };
        match store.select_login_profile(id) {
            Ok(()) => {
                self.profile = store.get(id);
                self.profile_error = None;
                self.cancel_login();
            }
            Err(e) => self.profile_error = Some(e),
        }
    }
    pub fn save_server(&mut self, base: String, allowed: String) {
        if self.profile_busy {
            return;
        }
        let Some(store) = self.profiles.clone() else {
            self.ensure_profiles();
            return;
        };
        let mut profile = match ServerProfile::new(base.trim()) {
            Ok(v) => v,
            Err(e) => {
                self.profile_error = Some(e);
                return;
            }
        };
        let previous = self.profile.as_ref().filter(|old| {
            url::Url::parse(&old.base)
                .ok()
                .and_then(|u| Origin::of(&u).ok())
                == url::Url::parse(&profile.base)
                    .ok()
                    .and_then(|u| Origin::of(&u).ok())
        });
        let expected = previous.map(|p| p.revision);
        if let Some(old) = previous {
            profile.id = old.id;
            profile.name = old.name.clone();
            profile.certificate = old.certificate.clone();
            profile.server_ca_pem = old.server_ca_pem.clone();
            profile.account_bound = old.account_bound;
        }
        for line in allowed.lines().map(str::trim).filter(|s| !s.is_empty()) {
            match Origin::parse(line) {
                Ok(origin) => {
                    if !profile.allowed.contains(&origin) {
                        profile.allowed.push(origin)
                    }
                }
                Err(e) => {
                    self.profile_error = Some(e);
                    return;
                }
            }
        }
        self.cancel_login();
        self.profile_busy = true;
        self.events.spawn(
            tokio::task::spawn_blocking(move || {
                let saved = store.save(profile, expected)?;
                if !saved.account_bound {
                    store.select_login_profile(saved.id)?;
                }
                Ok::<_, String>(saved)
            }),
            |core, result| {
                core.profile_busy = false;
                match result {
                    Ok(Ok(p)) => {
                        core.profile = Some(p);
                        core.profile_error = None;
                        core.notify("Server settings saved", false)
                    }
                    Ok(Err(e)) => core.profile_error = Some(e),
                    Err(_) => core.profile_error = Some("Could not save server settings".into()),
                }
            },
        );
    }
    pub fn import_certificate(&mut self, path: std::path::PathBuf, password: Zeroizing<String>) {
        if self.profile_busy {
            return;
        }
        let (Some(store), Some(profile)) = (self.profiles.clone(), self.profile.clone()) else {
            return;
        };
        let id = profile.id;
        let request = uuid::Uuid::new_v4();
        self.profile_request = Some(request);
        self.profile_busy = true;
        self.events.spawn(
            tokio::task::spawn_blocking(move || {
                certificate::import(&store, profile, &path, password)
            }),
            move |core, result| {
                if core.profile_request != Some(request) {
                    return;
                }
                core.profile_request = None;
                core.profile_busy = false;
                match result {
                    Ok(Ok(p)) => {
                        if core.profile.as_ref().is_some_and(|v| v.id == id) {
                            core.profile = Some(p);
                        }
                        core.profile_error = None;
                        core.notify("Client certificate imported", false)
                    }
                    Ok(Err(e)) => core.profile_error = Some(e),
                    Err(_) => core.profile_error = Some("Certificate import failed".into()),
                }
            },
        );
    }
    pub fn import_ca(&mut self, path: std::path::PathBuf) {
        if self.profile_busy {
            return;
        }
        let (Some(store), Some(mut profile)) = (self.profiles.clone(), self.profile.clone()) else {
            return;
        };
        self.profile_busy = true;
        let id = profile.id;
        self.events.spawn(
            tokio::task::spawn_blocking(move || {
                let pem = read_bounded(&path, 1024 * 1024)?;
                profile.server_ca_pem = Some(certificate::validate_ca(&pem)?);
                let revision = profile.revision;
                store.save(profile, Some(revision))
            }),
            move |core, result| {
                core.profile_busy = false;
                match result {
                    Ok(Ok(p)) => {
                        if core.profile.as_ref().is_some_and(|v| v.id == id) {
                            core.profile = Some(p);
                        }
                        core.profile_error = None;
                    }
                    Ok(Err(e)) => core.profile_error = Some(e),
                    Err(_) => core.profile_error = Some("CA import failed".into()),
                }
            },
        );
    }
    pub fn remove_certificate(&mut self, ca: bool) {
        if self.profile_busy {
            return;
        }
        let (Some(store), Some(mut profile)) = (self.profiles.clone(), self.profile.clone()) else {
            return;
        };
        if ca {
            profile.server_ca_pem = None;
        } else {
            profile.certificate = None;
        }
        let revision = profile.revision;
        match store.save(profile, Some(revision)) {
            Ok(p) => {
                self.profile = Some(p);
                self.profile_error = None;
            }
            Err(e) => self.profile_error = Some(e),
        }
    }
    pub fn probe_server(&mut self) {
        if self.profile_busy {
            return;
        }
        let (Some(store), Some(profile)) = (self.profiles.clone(), self.profile.clone()) else {
            return;
        };
        self.profile_busy = true;
        self.events.spawn(
            async move {
                let transport =
                    tokio::task::spawn_blocking(move || Transport::new(&store, profile))
                        .await
                        .map_err(|_| "Could not prepare connection test")??;

                let url = transport
                    .profile
                    .default_api()?
                    .join("config")
                    .map_err(|_| "Invalid server URL")?;
                let request = transport
                    .request(reqwest_account::Method::GET, url)?
                    .build()
                    .map_err(|_| "Invalid connection request")?;
                let response = transport.execute(request).await?;
                if response.status().is_success() {
                    Ok(())
                } else {
                    Err(format!("Server replied with {}", response.status()))
                }
            },
            |core, result: Result<(), String>| {
                core.profile_busy = false;
                match result {
                    Ok(()) => {
                        core.profile_error = None;
                        core.notify("Connection successful", false)
                    }
                    Err(e) => core.profile_error = Some(e),
                }
            },
        );
    }
}
impl Core {
    pub fn pick_certificate(&self, password: Zeroizing<String>) {
        let profile = self.profile.as_ref().map(|p| (p.id, p.revision));
        self.events.spawn(
            async {
                rfd::AsyncFileDialog::new()
                    .add_filter("PKCS#12", &["p12", "pfx"])
                    .pick_file()
                    .await
            },
            move |core, file| {
                if core.profile.as_ref().map(|p| (p.id, p.revision)) == profile
                    && let Some(file) = file
                {
                    core.import_certificate(file.path().into(), password);
                }
            },
        );
    }
    pub fn pick_ca(&self) {
        let profile = self.profile.as_ref().map(|p| (p.id, p.revision));
        self.events.spawn(
            async {
                rfd::AsyncFileDialog::new()
                    .add_filter("CA certificate", &["pem", "crt"])
                    .pick_file()
                    .await
            },
            move |core, file| {
                if core.profile.as_ref().map(|p| (p.id, p.revision)) == profile
                    && let Some(file) = file
                {
                    core.import_ca(file.path().into());
                }
            },
        );
    }
}
