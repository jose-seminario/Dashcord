#[cfg(not(any(test, feature = "test-mock")))]
use keyring::Entry;

const SERVICE_NAME: &str = "DashCord";
const TOKEN_USER: &str = "discord_user_token";

#[cfg(any(test, feature = "test-mock"))]
use std::collections::HashMap;
#[cfg(any(test, feature = "test-mock"))]
use std::sync::Mutex;

#[cfg(any(test, feature = "test-mock"))]
static MOCK_KEYRING: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub struct KeychainManager;

impl KeychainManager {
    /// Guarda el token de forma segura (o no-op en modo dry-run)
    pub fn store_token(token: &str, is_dry_run: bool) -> Result<(), String> {
        if is_dry_run {
            println!("[KEYCHAIN] Dry-run: token not saved");
            use std::io::Write;
            let _ = std::io::stdout().flush();
            return Ok(());
        }

        #[cfg(any(test, feature = "test-mock"))]
        {
            let mut guard = MOCK_KEYRING.lock().map_err(|e| e.to_string())?;
            let map = guard.get_or_insert_with(HashMap::new);
            map.insert(format!("{}:{}", SERVICE_NAME, TOKEN_USER), token.to_string());
            Ok(())
        }

        #[cfg(not(any(test, feature = "test-mock")))]
        {
            let entry = Entry::new(SERVICE_NAME, TOKEN_USER).map_err(|e| e.to_string())?;
            entry.set_password(token).map_err(|e| e.to_string())
        }
    }

    /// Obtiene el token guardado (lectura benigna en Credential Manager)
    pub fn get_token() -> Result<String, String> {
        #[cfg(any(test, feature = "test-mock"))]
        {
            let guard = MOCK_KEYRING.lock().map_err(|e| e.to_string())?;
            if let Some(map) = guard.as_ref() {
                if let Some(tok) = map.get(&format!("{}:{}", SERVICE_NAME, TOKEN_USER)) {
                    return Ok(tok.clone());
                }
            }
            Err("No token found in keyring".to_string())
        }

        #[cfg(not(any(test, feature = "test-mock")))]
        {
            let entry = Entry::new(SERVICE_NAME, TOKEN_USER).map_err(|e| e.to_string())?;
            entry.get_password().map_err(|e| e.to_string())
        }
    }

    /// Elimina el token del Keychain
    pub fn delete_token(is_dry_run: bool) -> Result<(), String> {
        if is_dry_run {
            println!("[KEYCHAIN] Dry-run: token not deleted");
            return Ok(());
        }

        #[cfg(any(test, feature = "test-mock"))]
        {
            let mut guard = MOCK_KEYRING.lock().map_err(|e| e.to_string())?;
            if let Some(map) = guard.as_mut() {
                map.remove(&format!("{}:{}", SERVICE_NAME, TOKEN_USER));
            }
            Ok(())
        }

        #[cfg(not(any(test, feature = "test-mock")))]
        {
            let entry = Entry::new(SERVICE_NAME, TOKEN_USER).map_err(|e| e.to_string())?;
            entry.delete_credential().map_err(|e| e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_load_delete_keychain_mock() {
        let test_token = "mfa.test_mock_token_secret_123456";

        // 1. Guardar token (no dry-run en tests)
        KeychainManager::store_token(test_token, false).expect("Error storing token in mock");

        // 2. Recuperar token
        let retrieved = KeychainManager::get_token().expect("Error retrieving token from mock");
        assert_eq!(retrieved, test_token);

        // 3. Eliminar token
        KeychainManager::delete_token(false).expect("Error deleting token from mock");

        // 4. Verificar que ya no existe
        assert!(KeychainManager::get_token().is_err());
    }

    #[test]
    fn test_dry_run_keychain_no_op() {
        // En dry-run store_token es no-op
        let res = KeychainManager::store_token("fake_token", true);
        assert!(res.is_ok());
    }
}
