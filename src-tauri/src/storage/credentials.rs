use keyring::Entry;

pub struct CredentialStore {
    service_name: String,
}

impl CredentialStore {
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_string(),
        }
    }

    pub fn set_secret(&self, account: &str, secret: &str) -> anyhow::Result<()> {
        let entry = Entry::new(&self.service_name, account)?;
        entry.set_password(secret)?;
        Ok(())
    }

    pub fn get_secret(&self, account: &str) -> anyhow::Result<Option<String>> {
        let entry = Entry::new(&self.service_name, account)?;
        match entry.get_password() {
            Ok(val) => Ok(Some(val)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn has_secret(&self, account: &str) -> anyhow::Result<bool> {
        Ok(self.get_secret(account)?.map(|s| !s.is_empty()).unwrap_or(false))
    }

    pub fn delete_secret(&self, account: &str) -> anyhow::Result<()> {
        let entry = Entry::new(&self.service_name, account)?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
