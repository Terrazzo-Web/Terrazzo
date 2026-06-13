use std::io::Read as _;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Instant;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use pbkdf2::hmac::Hmac;
use sha2::Sha256;
use sha2::digest::InvalidLength;
use trz_gateway_common::retry_strategy::RetryStrategy;

use super::ServerConfig;
use super::io::ConfigFileError;
use super::server::DynamicServerConfig;
use crate::backend::config::types::Password;

impl DynamicServerConfig {
    pub fn set_password(&self, password_stdin: bool) -> Result<(), SetPasswordError> {
        let password = if password_stdin {
            read_password_from_stdin()?
        } else {
            rpassword::prompt_password("Password: ")?
        };
        self.set_password_value(password.as_str())
    }

    fn set_password_value(&self, password: &str) -> Result<(), SetPasswordError> {
        let () = self.try_set(|server| {
            let password = server.hash_password(password)?;
            Ok::<_, SetPasswordError>(
                ServerConfig {
                    password: Some(password),
                    ..(**server).clone()
                }
                .into(),
            )
        })?;
        debug_assert!(matches!(self.get().verify_password(password), Ok(())));
        Ok(())
    }
}

fn read_password_from_stdin() -> Result<String, SetPasswordError> {
    let mut password = String::new();
    std::io::stdin().read_to_string(&mut password)?;
    Ok(normalize_password_input(&password).to_owned())
}

fn normalize_password_input(password: &str) -> &str {
    password.trim_end_matches(['\r', '\n'])
}

impl ServerConfig {
    fn hash_password(&self, password: &str) -> Result<Password, SetPasswordError> {
        let mut hash = [0u8; 20];
        let salt = uuid::Uuid::new_v4();
        let iterations = 60_000;
        let () = pbkdf2::pbkdf2::<Hmac<Sha256>>(
            password.as_bytes(),
            salt.as_bytes(),
            iterations,
            &mut hash,
        )?;
        Ok(Password {
            hash: hash.to_vec(),
            iterations,
            salt: salt.as_bytes().to_vec(),
        })
    }

    pub fn verify_password(&self, password: &str) -> Result<(), VerifyPasswordError> {
        let Some(password_hash) = &self.password else {
            return Err(VerifyPasswordError::PasswordNotDefined);
        };
        let mut state = PASSWORD_ATTEMPT_STATE.lock().unwrap();
        let now = Instant::now();
        if let Some(next_attempt_at) = state.next_attempt_at
            && next_attempt_at > now
        {
            return Err(VerifyPasswordError::InvalidPassword);
        }
        let mut hash = [0u8; 20];
        let () = pbkdf2::pbkdf2::<Hmac<Sha256>>(
            password.as_bytes(),
            &password_hash.salt,
            password_hash.iterations,
            &mut hash,
        )?;
        if hash.as_slice() == password_hash.hash.as_slice() {
            *state = PasswordAttemptState::new();
            Ok(())
        } else {
            let retry_delay = state.retry_strategy.delay();
            state.next_attempt_at = Some(Instant::now() + retry_delay);
            Err(VerifyPasswordError::InvalidPassword)
        }
    }
}

static PASSWORD_ATTEMPT_STATE: LazyLock<Mutex<PasswordAttemptState>> =
    LazyLock::new(|| Mutex::new(PasswordAttemptState::new()));

struct PasswordAttemptState {
    retry_strategy: RetryStrategy,
    next_attempt_at: Option<Instant>,
}

impl PasswordAttemptState {
    fn new() -> Self {
        Self {
            retry_strategy: RetryStrategy::default(),
            next_attempt_at: None,
        }
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum SetPasswordError {
    #[error("[{n}] Failed read password: {0}", n = self.name())]
    Prompt(#[from] std::io::Error),

    #[error("[{n}] Failed to save config file with password: {0}", n = self.name())]
    Save(#[from] ConfigFileError),

    #[error("[{n}] {0}", n = self.name())]
    Pbkdf2(#[from] InvalidLength),
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum VerifyPasswordError {
    #[error("[{n}] The password is not configured", n = self.name())]
    PasswordNotDefined,

    #[error("[{n}] The password doesn't match", n = self.name())]
    InvalidPassword,

    #[error("[{n}] {0}", n = self.name())]
    Pbkdf2(#[from] InvalidLength),
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;
    use std::time::Instant;

    use crate::backend::config::ServerConfig;
    use crate::backend::config::password::PASSWORD_ATTEMPT_STATE;
    use crate::backend::config::password::PasswordAttemptState;
    use crate::backend::config::password::VerifyPasswordError;
    use crate::backend::config::password::normalize_password_input;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_password_attempt_state() {
        let mut state = PASSWORD_ATTEMPT_STATE.lock().unwrap();
        *state = PasswordAttemptState::new()
    }

    fn test_config() -> ServerConfig {
        reset_password_attempt_state();
        let config = crate::backend::config::ConfigFile::default().merge(&Default::default());
        let config_file = (*config.server).clone();
        let password = config_file.hash_password("pa$$word").unwrap();
        ServerConfig {
            password: Some(password),
            ..config_file
        }
    }

    #[test]
    fn test_password() {
        let _test_lock = TEST_LOCK.lock().unwrap();
        let config_file = test_config();
        assert!(matches!(config_file.verify_password("pa$$word"), Ok(())));
        assert!(matches!(
            config_file.verify_password("pa$$word2"),
            Err(VerifyPasswordError::InvalidPassword)
        ));
    }

    #[test]
    fn wrong_password_blocks_immediate_retry() {
        let _test_lock = TEST_LOCK.lock().unwrap();
        let config_file = test_config();
        assert!(matches!(
            config_file.verify_password("pa$$word2"),
            Err(VerifyPasswordError::InvalidPassword)
        ));

        {
            let attempt_state = PASSWORD_ATTEMPT_STATE.lock().unwrap();
            assert!(attempt_state.next_attempt_at.unwrap() > Instant::now());
            assert_eq!(attempt_state.retry_strategy.peek(), Duration::from_secs(2));
        }

        assert!(matches!(
            config_file.verify_password("pa$$word"),
            Err(VerifyPasswordError::InvalidPassword)
        ));

        {
            let attempt_state = PASSWORD_ATTEMPT_STATE.lock().unwrap();
            assert_eq!(attempt_state.retry_strategy.peek(), Duration::from_secs(2));
            assert!(attempt_state.next_attempt_at.unwrap() > Instant::now());
        }
    }

    #[test]
    fn correct_password_resets_failed_attempts() {
        let _test_lock = TEST_LOCK.lock().unwrap();
        let config_file = test_config();
        assert!(matches!(
            config_file.verify_password("pa$$word2"),
            Err(VerifyPasswordError::InvalidPassword)
        ));
        reset_password_attempt_state();

        assert!(matches!(config_file.verify_password("pa$$word"), Ok(())));
        {
            let attempt_state = PASSWORD_ATTEMPT_STATE.lock().unwrap();
            assert_eq!(attempt_state.retry_strategy.peek(), Duration::from_secs(1));
            assert!(attempt_state.next_attempt_at.is_none());
        }
    }

    #[test]
    fn normalize_password_input_trims_newlines_only() {
        assert_eq!(normalize_password_input("pa$$word\n"), "pa$$word");
        assert_eq!(normalize_password_input("pa$$word\r\n"), "pa$$word");
        assert_eq!(normalize_password_input(" pa$$word \n"), " pa$$word ");
    }
}
