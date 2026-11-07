use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use secrecy::SecretString;

use super::vault::SecretVault;
use crate::errors::{AppError, Result};

pub(crate) struct VaultGate {
    entered: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
}

impl VaultGate {
    fn wait(&self) {
        if let Some(entered) = self.entered.lock().expect("gate sender").take() {
            entered.send(()).expect("test awaits vault entry");
            self.release
                .lock()
                .expect("gate receiver")
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("test releases vault");
        }
    }
}

pub(crate) fn vault_gate() -> (
    VaultGate,
    tokio::sync::oneshot::Receiver<()>,
    std::sync::mpsc::Sender<()>,
) {
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    (
        VaultGate {
            entered: Mutex::new(Some(entered)),
            release: Mutex::new(released),
        },
        waiting,
        release,
    )
}

type Failure = fn() -> AppError;

struct ReadTrap {
    free_reads: usize,
    gate: Option<VaultGate>,
    failure: Failure,
}

#[derive(Default)]
pub(crate) struct MemoryVault {
    secrets: Mutex<HashMap<String, SecretString>>,
    reads: Arc<AtomicUsize>,
    set_calls: Mutex<Vec<String>>,
    delete_calls: Mutex<Vec<String>>,
    read_trap: Option<ReadTrap>,
    write_failure: Option<Failure>,
    delete_failure: Option<Failure>,
    delete_gate: Option<VaultGate>,
    ephemeral: bool,
}

impl MemoryVault {
    pub(crate) fn with_secret(self, key: &str, value: &str) -> Self {
        self.seed(key, value);
        self
    }

    pub(crate) fn ephemeral(mut self) -> Self {
        self.ephemeral = true;
        self
    }

    pub(crate) fn failing_reads(mut self, failure: Failure) -> Self {
        self.read_trap = Some(ReadTrap {
            free_reads: 0,
            gate: None,
            failure,
        });
        self
    }

    pub(crate) fn gating_reads(
        mut self,
        free_reads: usize,
        gate: VaultGate,
        failure: Failure,
    ) -> Self {
        self.read_trap = Some(ReadTrap {
            free_reads,
            gate: Some(gate),
            failure,
        });
        self
    }

    pub(crate) fn failing_writes(mut self, failure: Failure) -> Self {
        self.write_failure = Some(failure);
        self
    }

    pub(crate) fn failing_deletes(mut self, failure: Failure) -> Self {
        self.delete_failure = Some(failure);
        self
    }

    pub(crate) fn gating_deletes(mut self, gate: VaultGate) -> Self {
        self.delete_gate = Some(gate);
        self
    }

    pub(crate) fn seed(&self, key: &str, value: &str) {
        self.secrets
            .lock()
            .expect("test vault")
            .insert(key.to_string(), SecretString::from(value.to_string()));
    }

    pub(crate) fn secret_count(&self) -> usize {
        self.secrets.lock().expect("test vault").len()
    }

    pub(crate) fn read_count(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }

    pub(crate) fn clear_reads(&self) {
        self.reads.store(0, Ordering::SeqCst);
    }

    pub(crate) fn read_counter(&self) -> Arc<AtomicUsize> {
        self.reads.clone()
    }

    pub(crate) fn set_calls(&self) -> Vec<String> {
        self.set_calls.lock().expect("test vault").clone()
    }

    pub(crate) fn delete_calls(&self) -> Vec<String> {
        self.delete_calls.lock().expect("test vault").clone()
    }
}

impl SecretVault for MemoryVault {
    fn survives_restart(&self) -> bool {
        !self.ephemeral
    }

    fn get_secret(&self, key: &str) -> Result<Option<SecretString>> {
        let read = self.reads.fetch_add(1, Ordering::SeqCst);
        if let Some(trap) = self
            .read_trap
            .as_ref()
            .filter(|trap| read >= trap.free_reads)
        {
            if let Some(gate) = &trap.gate {
                gate.wait();
            }
            return Err((trap.failure)());
        }
        Ok(self.secrets.lock().expect("test vault").get(key).cloned())
    }

    fn set_secret(&self, key: &str, value: SecretString) -> Result<()> {
        self.set_calls
            .lock()
            .expect("test vault")
            .push(key.to_string());
        if let Some(failure) = self.write_failure {
            return Err(failure());
        }
        self.secrets
            .lock()
            .expect("test vault")
            .insert(key.to_string(), value);
        Ok(())
    }

    fn delete_secret(&self, key: &str) -> Result<()> {
        self.delete_calls
            .lock()
            .expect("test vault")
            .push(key.to_string());
        if let Some(gate) = &self.delete_gate {
            gate.wait();
        }
        if let Some(failure) = self.delete_failure {
            return Err(failure());
        }
        self.secrets.lock().expect("test vault").remove(key);
        Ok(())
    }
}

pub(crate) fn vault_unavailable() -> AppError {
    AppError::General("vault unavailable".to_string())
}

pub(crate) fn keychain_locked() -> AppError {
    AppError::ResourceCleanup("keychain locked".to_string())
}
